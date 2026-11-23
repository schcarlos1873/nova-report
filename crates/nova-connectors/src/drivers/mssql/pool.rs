//! Pool de conexões do SQL Server.
//!
//! Sem pool, cada query (preview, parâmetros em cascata, introspecção) refaz TCP + TLS + login.
//! Aqui guardamos até [`MAX_IDLE_PER_KEY`] clientes ociosos por configuração (fingerprint do
//! perfil + senha). Um cliente ocioso há mais de [`PING_AFTER`] é validado com `SELECT 1`
//! antes do uso; após [`IDLE_TTL`] é descartado (firewalls/NAT costumam derrubar conexões
//! ociosas sem avisar).
//!
//! Para não vazar estado de sessão entre consultas, uma conexão só volta ao pool se a query
//! não mexe na sessão (veja [`reusable`]).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use nova_domain::connection::ConnectionConfig;

use super::MsClient;
use crate::{ConnectorResult, diag};

pub const MAX_IDLE_PER_KEY: usize = 4;
pub const IDLE_TTL: Duration = Duration::from_secs(300);
pub const PING_AFTER: Duration = Duration::from_secs(30);
const PING_TIMEOUT: Duration = Duration::from_secs(5);

struct Idle {
    client: MsClient,
    since: Instant,
}

#[derive(Default)]
pub struct MsPool {
    idle: Mutex<HashMap<String, Vec<Idle>>>,
}

impl MsPool {
    /// Retira um cliente ocioso válido ou abre uma conexão nova.
    pub async fn get(&self, cfg: &ConnectionConfig) -> ConnectorResult<MsClient> {
        let key = cfg.fingerprint();
        loop {
            let Some(Idle { mut client, since }) = self.take(&key) else { break };
            let idle_for = since.elapsed();
            if idle_for < PING_AFTER {
                diag::info(format!("Reutilizando conexão do pool (ociosa há {} s)", idle_for.as_secs()));
                return Ok(client);
            }
            if ping(&mut client).await {
                diag::info(format!("Conexão do pool validada (ociosa há {} s)", idle_for.as_secs()));
                return Ok(client);
            }
            diag::warn("Conexão ociosa do pool não respondeu: descartada");
        }
        super::connect(cfg).await
    }

    /// Devolve um cliente ao pool (descarta se o pool da chave estiver cheio).
    pub fn put(&self, cfg: &ConnectionConfig, client: MsClient) {
        let Ok(mut idle) = self.idle.lock() else { return };
        Self::purge(&mut idle);
        let list = idle.entry(cfg.fingerprint()).or_default();
        if list.len() < MAX_IDLE_PER_KEY {
            list.push(Idle { client, since: Instant::now() });
        }
    }

    fn take(&self, key: &str) -> Option<Idle> {
        let mut idle = self.idle.lock().ok()?;
        Self::purge(&mut idle);
        idle.get_mut(key)?.pop()
    }

    fn purge(idle: &mut HashMap<String, Vec<Idle>>) {
        idle.values_mut().for_each(|l| l.retain(|i| i.since.elapsed() < IDLE_TTL));
        idle.retain(|_, l| !l.is_empty());
    }
}

async fn ping(client: &mut MsClient) -> bool {
    let probe = async { client.simple_query("SELECT 1").await?.into_results().await };
    matches!(tokio::time::timeout(PING_TIMEOUT, probe).await, Ok(Ok(_)))
}

/// A conexão pode voltar ao pool depois desta query? Recusa (conservador) qualquer coisa que
/// possa deixar estado na sessão: tabelas temporárias (`#`), `SET`, `USE` e transações.
pub fn reusable(sql: &str) -> bool {
    if sql.contains('#') {
        return false;
    }
    let lower = sql.to_ascii_lowercase();
    !lower
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|t| matches!(t, "set" | "use" | "tran" | "transaction" | "commit" | "rollback" | "save"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuso_apenas_sem_estado_de_sessao() {
        assert!(reusable("SELECT * FROM pedidos WHERE id = @P1 ORDER BY data OFFSET 0 ROWS"));
        assert!(reusable("EXEC dbo.rel_vendas @P1, @P2"));
        assert!(reusable("SELECT settings, user_id FROM t"));
        assert!(!reusable("SELECT * INTO #tmp FROM t; SELECT * FROM #tmp"));
        assert!(!reusable("SET DATEFORMAT dmy; SELECT 1"));
        assert!(!reusable("USE outro_banco; SELECT 1"));
        assert!(!reusable("BEGIN TRAN; SELECT 1"));
    }
}

//! Microsoft SQL Server / Azure SQL (Tier 1, `tiberius` / TDS).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use nova_domain::ParamValue;
use nova_domain::connection::{ConnectionConfig, DatabaseTree};
use nova_domain::report::ProviderKind;
use nova_engine::EngineResult;
use nova_engine::data::{CellValue, DataProvider};
use polars::prelude::DataFrame;
use tiberius::{AuthMethod, Client, ColumnData, Config, EncryptionLevel, FromSql, ToSql};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::params::{PlaceholderStyle, bind_named};
use crate::registry::{Driver, DriverInfo};
use crate::rowset::RowSet;
use crate::{ConnectorError, ConnectorResult, NetErrorKind, diag, introspect, net};

mod browser;
mod pool;

pub(crate) type MsClient = Client<Compat<TcpStream>>;

/// Driver SQL Server com pool de conexões (veja [`pool`]).
#[derive(Default)]
pub struct MsSqlDriver {
    pool: Arc<pool::MsPool>,
}

/// Servidor alvo: host, instância nomeada e porta explícita.
#[derive(Debug, PartialEq)]
struct Target {
    host: String,
    instance: Option<String>,
    port: Option<u16>,
}

/// Interpreta `tcp:HOST\INSTANCIA,PORTA`, `HOST\INSTANCIA`, `HOST,PORTA` ou `HOST`.
fn parse_server(raw: &str) -> Target {
    let s = raw.trim().trim_matches(['{', '}', '"', '\'']);
    let s = s.get(..4).filter(|p| p.eq_ignore_ascii_case("tcp:")).map_or(s, |_| &s[4..]);
    let (s, port) = match s.rsplit_once(',') {
        Some((h, p)) => (h, p.trim().parse().ok()),
        None => (s, None),
    };
    let (host, instance) = match s.split_once('\\') {
        Some((h, i)) => (h, Some(i.trim().to_string()).filter(|i| !i.is_empty())),
        None => (s, None),
    };
    let host = match host.trim() {
        "" | "." | "(local)" | "(localdb)" => "localhost",
        h => h,
    };
    Target { host: host.to_string(), instance, port }
}

/// Valor de `Server`/`Data Source`/`Address` de uma connection string ADO.
fn ado_server(cs: &str) -> Option<String> {
    cs.split(';')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| {
            matches!(k.trim().to_ascii_lowercase().as_str(), "server" | "data source" | "address" | "addr" | "network address")
        })
        .map(|(_, v)| v.trim().to_string())
}

fn target(cfg: &ConnectionConfig) -> Target {
    if let Some(server) = cfg.resolved_connection_string().as_deref().and_then(ado_server) {
        return parse_server(&server);
    }
    let p = &cfg.profile;
    let mut t = parse_server(p.host.as_deref().unwrap_or("localhost"));
    if t.port.is_none() {
        // o formulário preenche 1433 por padrão: com instância nomeada só vale se o usuário trocou
        t.port = p.port.filter(|&port| t.instance.is_none() || port != 1433);
    }
    t
}

fn config(cfg: &ConnectionConfig) -> ConnectorResult<Config> {
    let p = &cfg.profile;
    let mut c = match cfg.resolved_connection_string() {
        Some(cs) => Config::from_ado_string(&cs).map_err(|e| ConnectorError::Config(e.to_string()))?,
        None => {
            let mut c = Config::new();
            if let Some(inst) = target(cfg).instance {
                c.instance_name(inst);
            }
            if let Some(db) = &p.database {
                c.database(db);
            }
            c.authentication(AuthMethod::sql_server(
                p.username.as_deref().unwrap_or("sa"),
                cfg.password.as_deref().unwrap_or(""),
            ));
            c
        }
    };
    c.application_name("NovaReport");
    c.encryption(if p.use_tls { EncryptionLevel::Required } else { EncryptionLevel::Off });
    if p.options.get("trustServerCertificate").is_some_and(|v| v == "true") || !p.use_tls {
        c.trust_cert();
    }
    Ok(c)
}

/// Abre uma conexão nova: (SQL Browser) → DNS → Happy Eyeballs → TLS/login, tudo sob `connectTimeout`.
async fn connect(cfg: &ConnectionConfig) -> ConnectorResult<MsClient> {
    let timeout = net::connect_timeout(cfg);
    let deadline = tokio::time::Instant::now() + timeout;
    let mut config = config(cfg)?;
    let t = target(cfg);
    let mut port = match (t.port, &t.instance) {
        (Some(p), Some(inst)) => {
            diag::info(format!("Porta {p} informada: SQL Browser não consultado para a instância '{inst}'"));
            p
        }
        (Some(p), None) => p,
        (None, Some(inst)) => browser::instance_port(&t.host, inst, deadline, timeout).await?,
        (None, None) => 1433,
    };
    let mut host = t.host;
    diag::info(format!(
        "SQL Server {host}:{port} · timeout {} s · TLS {}",
        timeout.as_secs(),
        if cfg.profile.use_tls { "exigido" } else { "só no login" }
    ));
    // O Azure SQL pode redirecionar para outro nó (`Routing`): segue uma vez.
    for attempt in 0..2 {
        config.host(&host);
        config.port(port);
        let tcp = net::connect_tcp(&host, port, deadline, timeout).await?.stream;
        diag::info("Handshake TDS/TLS e login…");
        let started = Instant::now();
        match tokio::time::timeout_at(deadline, Client::connect(config.clone(), tcp.compat_write())).await {
            Err(_) => {
                diag::error("Sem resposta do servidor durante TLS/login");
                return Err(net::timeout_error("autenticar (TLS/login)", &host, port, timeout, None));
            }
            Ok(Ok(client)) => {
                diag::ok(format!("Login aceito ({} ms)", started.elapsed().as_millis()));
                return Ok(client);
            }
            Ok(Err(tiberius::error::Error::Routing { host: h, port: p })) if attempt == 0 => {
                diag::info(format!("Servidor redirecionou para {h}:{p}"));
                host = h;
                port = p;
            }
            Ok(Err(e)) => {
                let err = diagnose(e, cfg, &host, port);
                diag::error(err.to_string());
                return Err(err);
            }
        }
    }
    Err(ConnectorError::Connection("o servidor redirecionou a conexão mais de uma vez".into()))
}

/// Traduz erros do `tiberius` em mensagens com dica.
fn diagnose(e: tiberius::error::Error, cfg: &ConnectionConfig, host: &str, port: u16) -> ConnectorError {
    use tiberius::error::Error as E;
    match e {
        E::Server(t) if t.code() == 18456 => ConnectorError::network(
            NetErrorKind::Auth,
            format!(
                "Login recusado para o usuário '{}'.",
                cfg.profile.username.as_deref().unwrap_or("sa")
            ),
            Some("confira usuário e senha; o servidor precisa aceitar autenticação SQL Server (modo misto).".into()),
        ),
        E::Server(t) if t.code() == 4060 => ConnectorError::network(
            NetErrorKind::Database,
            format!("Não foi possível abrir o banco '{}'.", cfg.profile.database.as_deref().unwrap_or("")),
            Some(format!("o banco não existe ou o usuário não tem acesso a ele ({}).", t.message())),
        ),
        E::Server(t) => ConnectorError::Connection(format!("{} (código {})", t.message(), t.code())),
        E::Tls(msg) => ConnectorError::network(
            NetErrorKind::Tls,
            format!("Falha no handshake TLS com {host}:{port}."),
            Some(format!(
                "se o certificado é autoassinado, use a opção trustServerCertificate=true; servidores antigos sem \
                 TLS 1.2 não são suportados — atualize o servidor ou desative o TLS. ({msg})"
            )),
        ),
        E::Io { kind: std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset, message } => {
            ConnectorError::network(
                NetErrorKind::Tls,
                format!("O servidor {host}:{port} encerrou a conexão durante o login."),
                Some(format!(
                    "pode ser incompatibilidade de TLS (servidor sem TLS 1.2) ou a porta não é de um SQL Server. ({message})"
                )),
            )
        }
        other => ConnectorError::Connection(other.to_string()),
    }
}

fn to_param(v: &ParamValue) -> Box<dyn ToSql> {
    match v {
        ParamValue::Int(i) => Box::new(*i),
        ParamValue::Float(f) => Box::new(*f),
        ParamValue::Bool(b) => Box::new(*b),
        ParamValue::Text(s) => Box::new(s.clone()),
        ParamValue::Date(d) => Box::new(*d),
        ParamValue::DateTime(d) => Box::new(*d),
        ParamValue::Null => Box::new(Option::<String>::None),
        // `bind_named` já expandiu listas em escalares; fallback defensivo
        ParamValue::List(_) => Box::new(v.to_text()),
    }
}

fn cell(data: &ColumnData<'static>) -> CellValue {
    match data {
        ColumnData::U8(v) => v.map_or(CellValue::Null, |x| CellValue::Int(x as i64)),
        ColumnData::I16(v) => v.map_or(CellValue::Null, |x| CellValue::Int(x as i64)),
        ColumnData::I32(v) => v.map_or(CellValue::Null, |x| CellValue::Int(x as i64)),
        ColumnData::I64(v) => v.map_or(CellValue::Null, CellValue::Int),
        ColumnData::F32(v) => v.map_or(CellValue::Null, |x| CellValue::Float(x as f64)),
        ColumnData::F64(v) => v.map_or(CellValue::Null, CellValue::Float),
        ColumnData::Bit(v) => v.map_or(CellValue::Null, CellValue::Bool),
        ColumnData::String(v) => v.as_ref().map_or(CellValue::Null, |s: &Cow<str>| CellValue::Text(s.to_string())),
        ColumnData::Guid(v) => v.map_or(CellValue::Null, |g| CellValue::Text(g.to_string())),
        ColumnData::Numeric(v) => v.map_or(CellValue::Null, |n| CellValue::Float(f64::from(n))),
        ColumnData::Binary(v) => v.as_ref().map_or(CellValue::Null, |b| crate::rowset::binary_cell(b)),
        ColumnData::Date(_) => chrono::NaiveDate::from_sql(data).ok().flatten().map_or(CellValue::Null, CellValue::Date),
        ColumnData::DateTime(_) | ColumnData::SmallDateTime(_) | ColumnData::DateTime2(_) => {
            chrono::NaiveDateTime::from_sql(data).ok().flatten().map_or(CellValue::Null, CellValue::DateTime)
        }
        ColumnData::DateTimeOffset(_) => chrono::DateTime::<chrono::Utc>::from_sql(data)
            .ok()
            .flatten()
            .map_or(CellValue::Null, |d| CellValue::DateTime(d.naive_utc())),
        ColumnData::Time(_) => chrono::NaiveTime::from_sql(data)
            .ok()
            .flatten()
            .map_or(CellValue::Null, |t| CellValue::Text(t.format("%H:%M:%S").to_string())),
        ColumnData::Xml(v) => v.as_ref().map_or(CellValue::Null, |x| CellValue::Text(x.to_string())),
    }
}

pub struct MsSqlProvider {
    config: ConnectionConfig,
    pool: Arc<pool::MsPool>,
}

impl MsSqlProvider {
    async fn run(&self, query: &str, params: &HashMap<String, ParamValue>) -> ConnectorResult<DataFrame> {
        let mut client = self.pool.get(&self.config).await?;
        let (sql, values) = bind_named(query, params, PlaceholderStyle::AtP);
        let boxed: Vec<Box<dyn ToSql>> = values.iter().map(to_param).collect();
        let refs: Vec<&dyn ToSql> = boxed.iter().map(|b| b.as_ref()).collect();
        let (columns, rows) = {
            let mut stream = client.query(sql, &refs).await.map_err(|e| ConnectorError::Query(e.to_string()))?;
            let columns: Vec<(String, Option<String>)> = stream
                .columns()
                .await
                .map_err(|e| ConnectorError::Query(e.to_string()))?
                .map(|cols| cols.iter().map(|c| (c.name().to_string(), Some(format!("{:?}", c.column_type())))).collect())
                .unwrap_or_default();
            let rows = stream.into_first_result().await.map_err(|e| ConnectorError::Query(e.to_string()))?;
            (columns, rows)
        };
        // em erro o cliente é descartado acima; aqui a sessão está limpa e o stream consumido
        if pool::reusable(query) {
            self.pool.put(&self.config, client);
        }
        let mut rs = RowSet::new(columns);
        for row in rows {
            rs.rows.push(row.into_iter().map(|d| cell(&d)).collect());
        }
        rs.into_dataframe()
    }
}

#[async_trait]
impl DataProvider for MsSqlProvider {
    async fn execute_query(&self, query: &str, params: &HashMap<String, ParamValue>) -> EngineResult<DataFrame> {
        self.run(query, params).await.map_err(Into::into)
    }
}

#[async_trait]
impl Driver for MsSqlDriver {
    fn info(&self) -> DriverInfo {
        DriverInfo {
            id: "mssql",
            name: "Microsoft SQL Server",
            tier: "1-nativo",
            default_port: Some(1433),
            fields: &["host", "port", "database", "username", "password", "useTls", "connectionString", "connectTimeout"],
            supports_introspection: true,
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Mssql
    }

    async fn test_connection(&self, cfg: &ConnectionConfig) -> ConnectorResult<u64> {
        let start = Instant::now();
        // sempre uma conexão nova: o teste precisa exercitar rede + login de verdade
        let mut client = connect(cfg).await?;
        client.simple_query("SELECT 1").await.map_err(|e| ConnectorError::Connection(e.to_string()))?
            .into_results().await.map_err(|e| ConnectorError::Connection(e.to_string()))?;
        Ok(super::elapsed_ms(start))
    }

    async fn introspect(&self, cfg: &ConnectionConfig) -> ConnectorResult<DatabaseTree> {
        let provider = MsSqlProvider { config: cfg.clone(), pool: self.pool.clone() };
        let db = cfg.profile.database.clone().unwrap_or_else(|| "master".into());
        introspect::run(
            &provider,
            &db,
            &introspect::information_schema_columns(""),
            Some(&introspect::information_schema_keys("")),
            Some(&introspect::information_schema_routines("")),
        )
        .await
    }

    fn provider(&self, config: ConnectionConfig) -> ConnectorResult<Arc<dyn DataProvider>> {
        Ok(Arc::new(MsSqlProvider { config, pool: self.pool.clone() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nova_domain::connection::ConnectionProfile;

    fn t(host: &str, instance: Option<&str>, port: Option<u16>) -> Target {
        Target { host: host.into(), instance: instance.map(Into::into), port }
    }

    #[test]
    fn interpreta_servidor() {
        assert_eq!(parse_server("SRV01\\SQLEXPRESS"), t("SRV01", Some("SQLEXPRESS"), None));
        assert_eq!(parse_server("tcp:db.local,1500"), t("db.local", None, Some(1500)));
        assert_eq!(parse_server("TCP:db\\INST,1600"), t("db", Some("INST"), Some(1600)));
        assert_eq!(parse_server(".\\SQLEXPRESS"), t("localhost", Some("SQLEXPRESS"), None));
        assert_eq!(parse_server("db.local"), t("db.local", None, None));
    }

    #[test]
    fn alvo_do_formulario_e_da_connection_string() {
        let mut p: ConnectionProfile =
            serde_json::from_value(serde_json::json!({"alias": "x", "provider": "mssql", "host": "SRV\\INST", "port": 1433})).unwrap();
        // 1433 é o padrão do formulário: com instância, consulta o SQL Browser
        assert_eq!(target(&ConnectionConfig::new(p.clone(), None)), t("SRV", Some("INST"), None));
        p.port = Some(1500);
        assert_eq!(target(&ConnectionConfig::new(p.clone(), None)), t("SRV", Some("INST"), Some(1500)));
        p.connection_string = Some("Server=tcp:outro\\X;Database=db;User Id=u;Password={PASSWORD}".into());
        assert_eq!(target(&ConnectionConfig::new(p, None)), t("outro", Some("X"), None));
    }
}

//! Modo diagnóstico: coleta, durante uma tentativa de conexão, cada etapa executada
//! (DNS, endereços descartados, tentativas TCP, SQL Browser, TLS/login...) com o tempo
//! decorrido, para o usuário copiar e enviar ao suporte.
//!
//! Fora de [`capture`] as etapas viram apenas `tracing::debug!` (custo zero na UI).

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagLevel {
    Info,
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagStep {
    /// Milissegundos desde o início da captura.
    pub t_ms: u64,
    pub level: DiagLevel,
    pub message: String,
}

struct Collector {
    start: Instant,
    steps: Mutex<Vec<DiagStep>>,
}

tokio::task_local! {
    static COLLECTOR: Arc<Collector>;
}

/// Registra uma etapa (no coletor ativo, se houver, e sempre no `tracing`).
pub fn step(level: DiagLevel, message: impl Into<String>) {
    let message = message.into();
    match level {
        DiagLevel::Warn | DiagLevel::Error => tracing::warn!("{message}"),
        _ => tracing::debug!("{message}"),
    }
    let _ = COLLECTOR.try_with(|c| {
        if let Ok(mut steps) = c.steps.lock() {
            steps.push(DiagStep { t_ms: c.start.elapsed().as_millis() as u64, level, message });
        }
    });
}

pub fn info(message: impl Into<String>) {
    step(DiagLevel::Info, message);
}
pub fn ok(message: impl Into<String>) {
    step(DiagLevel::Ok, message);
}
pub fn warn(message: impl Into<String>) {
    step(DiagLevel::Warn, message);
}
pub fn error(message: impl Into<String>) {
    step(DiagLevel::Error, message);
}

/// `true` quando há uma captura ativa (evita formatar mensagens caras à toa).
pub fn active() -> bool {
    COLLECTOR.try_with(|_| ()).is_ok()
}

/// Executa `fut` coletando as etapas registradas nele (inclusive as já registradas se o
/// future for cancelado: o coletor é compartilhado com quem chamou).
pub async fn capture<F: Future>(fut: F) -> (F::Output, Vec<DiagStep>) {
    let collector = Arc::new(Collector { start: Instant::now(), steps: Mutex::default() });
    let out = COLLECTOR.scope(collector.clone(), fut).await;
    let steps = collector.steps.lock().map(|s| s.clone()).unwrap_or_default();
    (out, steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn captura_apenas_dentro_do_escopo() {
        info("fora");
        let (v, steps) = capture(async {
            info("a");
            warn("b");
            7
        })
        .await;
        assert_eq!(v, 7);
        assert_eq!(steps.iter().map(|s| s.message.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(steps[1].level, DiagLevel::Warn);
        assert!(!active());
    }
}

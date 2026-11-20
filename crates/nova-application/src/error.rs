use nova_domain::DomainError;
use nova_engine::EngineError;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("conector: {0}")]
    Connector(String),
    #[error("{0}")]
    Invalid(String),
    #[error("tarefa interrompida: {0}")]
    Join(String),
}

impl AppError {
    /// Mensagem amigável incluindo detalhes de validação.
    pub fn user_message(&self) -> String {
        match self {
            AppError::Domain(DomainError::Validation(issues)) => {
                let lines: Vec<String> = issues.iter().map(|i| format!("• {}: {}", i.path, i.message)).collect();
                format!("O relatório possui erros:\n{}", lines.join("\n"))
            }
            other => other.to_string(),
        }
    }
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.user_message())
    }
}

use thiserror::Error;

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum DomainError {
    #[error("definição de relatório inválida: {0}")]
    InvalidReport(String),
    #[error("falha de validação ({} problema(s))", .0.len())]
    Validation(Vec<crate::report::ValidationIssue>),
    #[error("parâmetro '{name}' inválido: {reason}")]
    InvalidParameter { name: String, reason: String },
    #[error("parâmetro obrigatório '{0}' não informado")]
    MissingParameter(String),
    #[error("versão de schema não suportada: {0}")]
    UnsupportedSchemaVersion(String),
    #[error("conexão '{0}' não encontrada")]
    ConnectionNotFound(String),
    #[error("serialização: {0}")]
    Serialization(String),
    #[error("repositório: {0}")]
    Repository(String),
    #[error("cofre de credenciais: {0}")]
    Vault(String),
}

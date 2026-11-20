use thiserror::Error;

pub type EngineResult<T> = Result<T, EngineError>;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error(transparent)]
    Domain(#[from] nova_domain::DomainError),
    #[error("fonte de dados: {0}")]
    DataSource(String),
    #[error("dataset '{0}' não carregado")]
    DatasetNotFound(String),
    #[error("expressão `{expression}`: {message}")]
    Expression { expression: String, message: String },
    #[error("compilação Typst falhou: {0}")]
    Compilation(String),
    #[error("recurso '{0}' não encontrado")]
    Asset(String),
    #[error("exportação: {0}")]
    Export(String),
    #[error(transparent)]
    Polars(#[from] polars::error::PolarsError),
}

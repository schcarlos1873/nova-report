use thiserror::Error;

pub type ConnectorResult<T> = Result<T, ConnectorError>;

#[derive(Debug, Error)]
pub enum ConnectorError {
    #[error("driver '{0}' não disponível nesta build")]
    DriverUnavailable(String),
    #[error("configuração inválida: {0}")]
    Config(String),
    #[error("falha de conexão: {0}")]
    Connection(String),
    #[error("falha na query: {0}")]
    Query(String),
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Engine(#[from] nova_engine::EngineError),
    #[error(transparent)]
    Domain(#[from] nova_domain::DomainError),
}

impl From<ConnectorError> for nova_engine::EngineError {
    fn from(e: ConnectorError) -> Self {
        match e {
            ConnectorError::Engine(inner) => inner,
            other => nova_engine::EngineError::DataSource(other.to_string()),
        }
    }
}

impl From<polars::error::PolarsError> for ConnectorError {
    fn from(e: polars::error::PolarsError) -> Self {
        ConnectorError::Engine(e.into())
    }
}

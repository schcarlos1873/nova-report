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
    /// Falha de rede/login já diagnosticada, com mensagem pronta para a UI.
    #[error("{message}{}", fmt_hint(.hint))]
    Network { kind: NetErrorKind, message: String, hint: Option<String> },
    #[error("falha na query: {0}")]
    Query(String),
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Engine(#[from] nova_engine::EngineError),
    #[error(transparent)]
    Domain(#[from] nova_domain::DomainError),
}

/// Categoria de uma falha de conexão (usada para escolher a dica exibida ao usuário).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetErrorKind {
    /// Nome não resolve ou só devolve endereços inutilizáveis.
    Dns,
    /// Porta fechada no servidor (RST).
    Refused,
    /// Host/rede inalcançável ou sem resposta (firewall, VPN, servidor desligado).
    Unreachable,
    /// Prazo de conexão (`connectTimeout`) estourado.
    Timeout,
    /// Falha no handshake TLS.
    Tls,
    /// Usuário/senha recusados.
    Auth,
    /// Banco inexistente ou sem permissão.
    Database,
}

fn fmt_hint(hint: &Option<String>) -> String {
    hint.as_ref().map(|h| format!("\nDica: {h}")).unwrap_or_default()
}

impl ConnectorError {
    pub fn network(kind: NetErrorKind, message: impl Into<String>, hint: Option<String>) -> Self {
        ConnectorError::Network { kind, message: message.into(), hint }
    }
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

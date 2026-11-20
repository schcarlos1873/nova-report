//! Registro de drivers: `ProviderKind` → implementação concreta.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use nova_domain::connection::{ConnectionConfig, DatabaseTree};
use nova_domain::report::ProviderKind;
use nova_engine::data::DataProvider;
use serde::Serialize;

use crate::dialect::{self, SqlDialect};
use crate::{ConnectorError, ConnectorResult};

/// Metadados do driver exibidos no diálogo de conexão.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub tier: &'static str,
    pub default_port: Option<u16>,
    /// Campos relevantes no formulário: host, port, database, username, password, path, connectionString, useTls.
    pub fields: &'static [&'static str],
    pub supports_introspection: bool,
}

#[async_trait]
pub trait Driver: Send + Sync {
    fn info(&self) -> DriverInfo;

    fn kind(&self) -> ProviderKind;

    fn dialect(&self) -> &'static dyn SqlDialect {
        dialect::for_provider(self.kind())
    }

    /// Abre uma conexão e executa um round-trip; retorna a latência em ms.
    async fn test_connection(&self, config: &ConnectionConfig) -> ConnectorResult<u64>;

    async fn introspect(&self, config: &ConnectionConfig) -> ConnectorResult<DatabaseTree>;

    fn provider(&self, config: ConnectionConfig) -> ConnectorResult<Arc<dyn DataProvider>>;
}

#[derive(Clone, Default)]
pub struct ConnectorRegistry {
    drivers: HashMap<ProviderKind, Arc<dyn Driver>>,
}

impl ConnectorRegistry {
    /// Registro com todos os drivers compilados nesta build.
    pub fn with_builtin_drivers() -> Self {
        use crate::drivers;
        let mut r = Self::default();
        r.register(Arc::new(drivers::sqlite::SqliteDriver));
        r.register(Arc::new(drivers::file::CsvDriver));
        r.register(Arc::new(drivers::file::JsonDriver));
        #[cfg(feature = "postgres")]
        r.register(Arc::new(drivers::postgres::PostgresDriver::default()));
        #[cfg(feature = "mysql")]
        r.register(Arc::new(drivers::mysql::MySqlDriver::default()));
        #[cfg(feature = "mssql")]
        r.register(Arc::new(drivers::mssql::MsSqlDriver));
        #[cfg(feature = "rest")]
        r.register(Arc::new(drivers::rest::RestDriver));
        #[cfg(feature = "odbc")]
        r.register(Arc::new(drivers::odbc::OdbcDriver));
        r
    }

    pub fn register(&mut self, driver: Arc<dyn Driver>) {
        self.drivers.insert(driver.kind(), driver);
    }

    pub fn get(&self, kind: ProviderKind) -> ConnectorResult<Arc<dyn Driver>> {
        self.drivers.get(&kind).cloned().ok_or_else(|| ConnectorError::DriverUnavailable(kind.id().into()))
    }

    pub fn list(&self) -> Vec<DriverInfo> {
        let mut v: Vec<_> = self.drivers.values().map(|d| d.info()).collect();
        v.sort_by_key(|d| (d.tier, d.name));
        v
    }
}

//! Adapter: expõe o [`ConnectorRegistry`] como port `ConnectorGateway` da aplicação.

use std::sync::Arc;

use async_trait::async_trait;
use nova_application::ports::{ConnectorGateway, DriverDescriptor};
use nova_application::{AppError, AppResult};
use nova_domain::connection::{ConnectionConfig, DatabaseTree};
use nova_domain::report::ProviderKind;
use nova_engine::data::DataProvider;

use crate::{ConnectorError, ConnectorRegistry, dialect};

impl From<ConnectorError> for AppError {
    fn from(e: ConnectorError) -> Self {
        match e {
            ConnectorError::Engine(inner) => AppError::Engine(inner),
            ConnectorError::Domain(inner) => AppError::Domain(inner),
            other => AppError::Connector(other.to_string()),
        }
    }
}

#[async_trait]
impl ConnectorGateway for ConnectorRegistry {
    fn drivers(&self) -> Vec<DriverDescriptor> {
        self.list()
            .into_iter()
            .map(|d| DriverDescriptor {
                id: d.id.into(),
                name: d.name.into(),
                tier: d.tier.into(),
                default_port: d.default_port,
                fields: d.fields.iter().map(|f| f.to_string()).collect(),
                supports_introspection: d.supports_introspection,
            })
            .collect()
    }

    fn provider(&self, config: ConnectionConfig) -> AppResult<Arc<dyn DataProvider>> {
        Ok(self.get(config.profile.provider)?.provider(config)?)
    }

    async fn test_connection(&self, config: &ConnectionConfig) -> AppResult<u64> {
        Ok(self.get(config.profile.provider)?.test_connection(config).await?)
    }

    async fn introspect(&self, config: &ConnectionConfig) -> AppResult<DatabaseTree> {
        Ok(self.get(config.profile.provider)?.introspect(config).await?)
    }

    fn paginate(&self, kind: ProviderKind, sql: &str, limit: usize) -> String {
        dialect::for_provider(kind).apply_pagination(sql, limit, 0)
    }

    fn select_all(&self, kind: ProviderKind, schema: Option<&str>, table: &str) -> String {
        dialect::for_provider(kind).select_all(schema, table)
    }
}

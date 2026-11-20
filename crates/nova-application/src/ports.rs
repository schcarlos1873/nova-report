//! Ports da camada de aplicação, implementados pela infraestrutura (`nova-connectors`).

use std::sync::Arc;

use async_trait::async_trait;
use nova_domain::connection::{ConnectionConfig, DatabaseTree};
use nova_domain::report::ProviderKind;
use nova_engine::data::DataProvider;
use serde::Serialize;

use crate::AppResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverDescriptor {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub default_port: Option<u16>,
    pub fields: Vec<String>,
    pub supports_introspection: bool,
}

/// Acesso aos drivers de dados.
#[async_trait]
pub trait ConnectorGateway: Send + Sync {
    fn drivers(&self) -> Vec<DriverDescriptor>;
    fn provider(&self, config: ConnectionConfig) -> AppResult<Arc<dyn DataProvider>>;
    async fn test_connection(&self, config: &ConnectionConfig) -> AppResult<u64>;
    async fn introspect(&self, config: &ConnectionConfig) -> AppResult<DatabaseTree>;
    /// Envolve a query com paginação no dialeto do driver.
    fn paginate(&self, kind: ProviderKind, sql: &str, limit: usize) -> String;
    fn select_all(&self, kind: ProviderKind, schema: Option<&str>, table: &str) -> String;
}

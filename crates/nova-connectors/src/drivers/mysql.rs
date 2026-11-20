//! MySQL / MariaDB (Tier 1, `sqlx`).

use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use nova_domain::connection::{ConnectionConfig, DatabaseTree};
use nova_domain::report::ProviderKind;
use nova_engine::data::DataProvider;
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlSslMode};

use super::sqlx_common::{PoolCache, sqlx_provider};
use crate::params::PlaceholderStyle;
use crate::registry::{Driver, DriverInfo};
use crate::{ConnectorError, ConnectorResult, introspect};

sqlx_provider!(MySqlProvider, MySqlPool, PlaceholderStyle::Question);

#[derive(Default)]
pub struct MySqlDriver {
    pools: PoolCache<MySqlPool>,
}

fn options(config: &ConnectionConfig) -> ConnectorResult<MySqlConnectOptions> {
    let p = &config.profile;
    let mut opts = match config.resolved_connection_string() {
        Some(cs) => MySqlConnectOptions::from_str(&cs).map_err(|e| ConnectorError::Config(e.to_string()))?,
        None => {
            let mut o = MySqlConnectOptions::new()
                .host(p.host.as_deref().unwrap_or("localhost"))
                .port(p.port.unwrap_or(3306));
            if let Some(db) = &p.database {
                o = o.database(db);
            }
            if let Some(u) = &p.username {
                o = o.username(u);
            }
            if let Some(pw) = &config.password {
                o = o.password(pw);
            }
            o
        }
    };
    opts = opts
        .ssl_mode(if p.use_tls { MySqlSslMode::Required } else { MySqlSslMode::Preferred })
        .charset("utf8mb4");
    Ok(opts)
}

impl MySqlDriver {
    async fn pool(&self, config: &ConnectionConfig) -> ConnectorResult<MySqlPool> {
        let key = config.fingerprint();
        let mut pools = self.pools.lock().await;
        if let Some(p) = pools.get(&key) {
            return Ok(p.clone());
        }
        let pool = MySqlPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(15))
            .connect_with(options(config)?)
            .await
            .map_err(|e| ConnectorError::Connection(e.to_string()))?;
        pools.insert(key, pool.clone());
        Ok(pool)
    }
}

#[async_trait]
impl Driver for MySqlDriver {
    fn info(&self) -> DriverInfo {
        DriverInfo {
            id: "mysql",
            name: "MySQL / MariaDB",
            tier: "1-nativo",
            default_port: Some(3306),
            fields: &["host", "port", "database", "username", "password", "useTls", "connectionString"],
            supports_introspection: true,
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Mysql
    }

    async fn test_connection(&self, config: &ConnectionConfig) -> ConnectorResult<u64> {
        let start = Instant::now();
        let pool = self.pool(config).await?;
        sqlx::query("SELECT 1").execute(&pool).await.map_err(|e| ConnectorError::Connection(e.to_string()))?;
        Ok(super::elapsed_ms(start))
    }

    async fn introspect(&self, config: &ConnectionConfig) -> ConnectorResult<DatabaseTree> {
        let provider = MySqlProvider { pool: self.pool(config).await? };
        let db = config.profile.database.clone().unwrap_or_else(|| "mysql".into());
        introspect::run(
            &provider,
            &db,
            &introspect::information_schema_columns("AND c.TABLE_SCHEMA = DATABASE()"),
            Some(&introspect::information_schema_keys("AND c.TABLE_SCHEMA = DATABASE()")),
            Some(&introspect::information_schema_routines("AND c.TABLE_SCHEMA = DATABASE()")),
        )
        .await
    }

    fn provider(&self, config: ConnectionConfig) -> ConnectorResult<Arc<dyn DataProvider>> {
        Ok(Arc::new(LazyMySql { driver_pools: self.pools.clone(), config }))
    }
}

/// Provider que obtém (ou cria) o pool na primeira execução.
struct LazyMySql {
    driver_pools: PoolCache<MySqlPool>,
    config: ConnectionConfig,
}

#[async_trait]
impl DataProvider for LazyMySql {
    async fn execute_query(
        &self,
        query: &str,
        params: &std::collections::HashMap<String, nova_domain::ParamValue>,
    ) -> nova_engine::EngineResult<polars::prelude::DataFrame> {
        let driver = MySqlDriver { pools: self.driver_pools.clone() };
        let pool = driver.pool(&self.config).await?;
        MySqlProvider { pool }.execute_query(query, params).await
    }
}

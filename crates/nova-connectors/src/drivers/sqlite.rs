//! SQLite (Tier 1, `rusqlite` com SQLite embutido). Ideal para arquivos locais e cache.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use nova_domain::ParamValue;
use nova_domain::connection::{ConnectionConfig, DatabaseTree};
use nova_domain::report::ProviderKind;
use nova_engine::EngineResult;
use nova_engine::data::{CellValue, DataProvider};
use polars::prelude::DataFrame;
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OpenFlags};

use crate::params::{PlaceholderStyle, bind_named};
use crate::registry::{Driver, DriverInfo};
use crate::rowset::RowSet;
use crate::{ConnectorError, ConnectorResult, introspect};

pub struct SqliteDriver;

fn db_path(config: &ConnectionConfig) -> ConnectorResult<PathBuf> {
    let p = config
        .profile
        .path
        .clone()
        .or_else(|| config.profile.database.clone())
        .or_else(|| config.resolved_connection_string().map(|s| s.trim_start_matches("sqlite://").to_string()))
        .ok_or_else(|| ConnectorError::Config("informe o caminho do arquivo SQLite".into()))?;
    Ok(PathBuf::from(p))
}

fn open(path: &PathBuf) -> ConnectorResult<Connection> {
    if !path.exists() {
        return Err(ConnectorError::Connection(format!("arquivo não encontrado: {}", path.display())));
    }
    // somente leitura: relatórios nunca alteram dados
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI)
        .map_err(|e| ConnectorError::Connection(e.to_string()))
}

fn to_sql_value(v: &ParamValue) -> Value {
    match v {
        ParamValue::Int(i) => Value::Integer(*i),
        ParamValue::Float(f) => Value::Real(*f),
        ParamValue::Bool(b) => Value::Integer(*b as i64),
        ParamValue::Null => Value::Null,
        other => Value::Text(other.to_text()),
    }
}

pub(crate) fn query_blocking(path: &PathBuf, sql: &str, params: &HashMap<String, ParamValue>) -> ConnectorResult<DataFrame> {
    let conn = open(path)?;
    let (sql, values) = bind_named(sql, params, PlaceholderStyle::QuestionNumbered);
    let mut stmt = conn.prepare(&sql).map_err(|e| ConnectorError::Query(e.to_string()))?;
    let columns: Vec<(String, Option<String>)> =
        stmt.columns().iter().map(|c| (c.name().to_string(), c.decl_type().map(str::to_string))).collect();
    let n = columns.len();
    let mut rs = RowSet::new(columns);
    let bound: Vec<Value> = values.iter().map(to_sql_value).collect();
    let mut rows = stmt
        .query(rusqlite::params_from_iter(bound.iter()))
        .map_err(|e| ConnectorError::Query(e.to_string()))?;
    while let Some(row) = rows.next().map_err(|e| ConnectorError::Query(e.to_string()))? {
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(match row.get_ref(i).map_err(|e| ConnectorError::Query(e.to_string()))? {
                ValueRef::Null => CellValue::Null,
                ValueRef::Integer(v) => CellValue::Int(v),
                ValueRef::Real(v) => CellValue::Float(v),
                ValueRef::Text(t) => CellValue::Text(String::from_utf8_lossy(t).into_owned()),
                ValueRef::Blob(b) => crate::rowset::binary_cell(b),
            });
        }
        rs.rows.push(out);
    }
    rs.into_dataframe()
}

pub struct SqliteProvider {
    path: PathBuf,
}

impl SqliteProvider {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl DataProvider for SqliteProvider {
    async fn execute_query(&self, query: &str, params: &HashMap<String, ParamValue>) -> EngineResult<DataFrame> {
        let (path, sql, params) = (self.path.clone(), query.to_string(), params.clone());
        tokio::task::spawn_blocking(move || query_blocking(&path, &sql, &params))
            .await
            .map_err(|e| nova_engine::EngineError::DataSource(e.to_string()))?
            .map_err(Into::into)
    }
}

#[async_trait]
impl Driver for SqliteDriver {
    fn info(&self) -> DriverInfo {
        DriverInfo {
            id: "sqlite",
            name: "SQLite",
            tier: "1-nativo",
            default_port: None,
            fields: &["path"],
            supports_introspection: true,
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Sqlite
    }

    async fn test_connection(&self, config: &ConnectionConfig) -> ConnectorResult<u64> {
        let path = db_path(config)?;
        let start = Instant::now();
        tokio::task::spawn_blocking(move || {
            open(&path)?.query_row("SELECT 1", [], |_| Ok(())).map_err(|e| ConnectorError::Connection(e.to_string()))
        })
        .await
        .map_err(|e| ConnectorError::Connection(e.to_string()))??;
        Ok(super::elapsed_ms(start))
    }

    async fn introspect(&self, config: &ConnectionConfig) -> ConnectorResult<DatabaseTree> {
        let path = db_path(config)?;
        let name = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let provider = SqliteProvider::new(path);
        introspect::run(&provider, &name, introspect::SQLITE_CATALOG, Some(introspect::SQLITE_FOREIGN_KEYS), None).await
    }

    fn provider(&self, config: ConnectionConfig) -> ConnectorResult<Arc<dyn DataProvider>> {
        Ok(Arc::new(SqliteProvider::new(db_path(&config)?)))
    }
}

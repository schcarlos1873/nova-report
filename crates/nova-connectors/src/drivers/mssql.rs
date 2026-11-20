//! Microsoft SQL Server / Azure SQL (Tier 1, `tiberius` / TDS).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use nova_domain::ParamValue;
use nova_domain::connection::{ConnectionConfig, DatabaseTree};
use nova_domain::report::ProviderKind;
use nova_engine::EngineResult;
use nova_engine::data::{CellValue, DataProvider};
use polars::prelude::DataFrame;
use tiberius::{AuthMethod, Client, ColumnData, Config, EncryptionLevel, FromSql, ToSql};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::params::{PlaceholderStyle, bind_named};
use crate::registry::{Driver, DriverInfo};
use crate::rowset::RowSet;
use crate::{ConnectorError, ConnectorResult, introspect};

pub struct MsSqlDriver;

fn config(cfg: &ConnectionConfig) -> ConnectorResult<Config> {
    let p = &cfg.profile;
    let mut c = match cfg.resolved_connection_string() {
        Some(cs) => Config::from_ado_string(&cs).map_err(|e| ConnectorError::Config(e.to_string()))?,
        None => {
            let mut c = Config::new();
            c.host(p.host.as_deref().unwrap_or("localhost"));
            c.port(p.port.unwrap_or(1433));
            if let Some(db) = &p.database {
                c.database(db);
            }
            c.authentication(AuthMethod::sql_server(
                p.username.as_deref().unwrap_or("sa"),
                cfg.password.as_deref().unwrap_or(""),
            ));
            c
        }
    };
    c.application_name("NovaReport");
    c.encryption(if p.use_tls { EncryptionLevel::Required } else { EncryptionLevel::Off });
    if p.options.get("trustServerCertificate").is_some_and(|v| v == "true") || !p.use_tls {
        c.trust_cert();
    }
    Ok(c)
}

async fn connect(cfg: &ConnectionConfig) -> ConnectorResult<Client<Compat<TcpStream>>> {
    let config = config(cfg)?;
    let tcp = TcpStream::connect(config.get_addr()).await.map_err(|e| ConnectorError::Connection(e.to_string()))?;
    tcp.set_nodelay(true)?;
    Client::connect(config, tcp.compat_write()).await.map_err(|e| ConnectorError::Connection(e.to_string()))
}

fn to_param(v: &ParamValue) -> Box<dyn ToSql> {
    match v {
        ParamValue::Int(i) => Box::new(*i),
        ParamValue::Float(f) => Box::new(*f),
        ParamValue::Bool(b) => Box::new(*b),
        ParamValue::Text(s) => Box::new(s.clone()),
        ParamValue::Date(d) => Box::new(*d),
        ParamValue::DateTime(d) => Box::new(*d),
        ParamValue::Null => Box::new(Option::<String>::None),
        // `bind_named` já expandiu listas em escalares; fallback defensivo
        ParamValue::List(_) => Box::new(v.to_text()),
    }
}

fn cell(data: &ColumnData<'static>) -> CellValue {
    match data {
        ColumnData::U8(v) => v.map_or(CellValue::Null, |x| CellValue::Int(x as i64)),
        ColumnData::I16(v) => v.map_or(CellValue::Null, |x| CellValue::Int(x as i64)),
        ColumnData::I32(v) => v.map_or(CellValue::Null, |x| CellValue::Int(x as i64)),
        ColumnData::I64(v) => v.map_or(CellValue::Null, CellValue::Int),
        ColumnData::F32(v) => v.map_or(CellValue::Null, |x| CellValue::Float(x as f64)),
        ColumnData::F64(v) => v.map_or(CellValue::Null, CellValue::Float),
        ColumnData::Bit(v) => v.map_or(CellValue::Null, CellValue::Bool),
        ColumnData::String(v) => v.as_ref().map_or(CellValue::Null, |s: &Cow<str>| CellValue::Text(s.to_string())),
        ColumnData::Guid(v) => v.map_or(CellValue::Null, |g| CellValue::Text(g.to_string())),
        ColumnData::Numeric(v) => v.map_or(CellValue::Null, |n| CellValue::Float(f64::from(n))),
        ColumnData::Binary(v) => v.as_ref().map_or(CellValue::Null, |b| CellValue::Text(format!("<binário {} bytes>", b.len()))),
        ColumnData::Date(_) => chrono::NaiveDate::from_sql(data).ok().flatten().map_or(CellValue::Null, CellValue::Date),
        ColumnData::DateTime(_) | ColumnData::SmallDateTime(_) | ColumnData::DateTime2(_) => {
            chrono::NaiveDateTime::from_sql(data).ok().flatten().map_or(CellValue::Null, CellValue::DateTime)
        }
        ColumnData::DateTimeOffset(_) => chrono::DateTime::<chrono::Utc>::from_sql(data)
            .ok()
            .flatten()
            .map_or(CellValue::Null, |d| CellValue::DateTime(d.naive_utc())),
        ColumnData::Time(_) => chrono::NaiveTime::from_sql(data)
            .ok()
            .flatten()
            .map_or(CellValue::Null, |t| CellValue::Text(t.format("%H:%M:%S").to_string())),
        ColumnData::Xml(v) => v.as_ref().map_or(CellValue::Null, |x| CellValue::Text(x.to_string())),
    }
}

pub struct MsSqlProvider {
    config: ConnectionConfig,
}

#[async_trait]
impl DataProvider for MsSqlProvider {
    async fn execute_query(&self, query: &str, params: &HashMap<String, ParamValue>) -> EngineResult<DataFrame> {
        let run = async {
            let mut client = connect(&self.config).await?;
            let (sql, values) = bind_named(query, params, PlaceholderStyle::AtP);
            let boxed: Vec<Box<dyn ToSql>> = values.iter().map(to_param).collect();
            let refs: Vec<&dyn ToSql> = boxed.iter().map(|b| b.as_ref()).collect();
            let mut stream = client.query(sql, &refs).await.map_err(|e| ConnectorError::Query(e.to_string()))?;
            let columns: Vec<(String, Option<String>)> = stream
                .columns()
                .await
                .map_err(|e| ConnectorError::Query(e.to_string()))?
                .map(|cols| cols.iter().map(|c| (c.name().to_string(), Some(format!("{:?}", c.column_type())))).collect())
                .unwrap_or_default();
            let rows = stream.into_first_result().await.map_err(|e| ConnectorError::Query(e.to_string()))?;
            let mut rs = RowSet::new(columns);
            for row in rows {
                rs.rows.push(row.into_iter().map(|d| cell(&d)).collect());
            }
            rs.into_dataframe()
        };
        run.await.map_err(Into::into)
    }
}

#[async_trait]
impl Driver for MsSqlDriver {
    fn info(&self) -> DriverInfo {
        DriverInfo {
            id: "mssql",
            name: "Microsoft SQL Server",
            tier: "1-nativo",
            default_port: Some(1433),
            fields: &["host", "port", "database", "username", "password", "useTls", "connectionString"],
            supports_introspection: true,
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Mssql
    }

    async fn test_connection(&self, cfg: &ConnectionConfig) -> ConnectorResult<u64> {
        let start = Instant::now();
        let mut client = connect(cfg).await?;
        client.simple_query("SELECT 1").await.map_err(|e| ConnectorError::Connection(e.to_string()))?
            .into_results().await.map_err(|e| ConnectorError::Connection(e.to_string()))?;
        Ok(super::elapsed_ms(start))
    }

    async fn introspect(&self, cfg: &ConnectionConfig) -> ConnectorResult<DatabaseTree> {
        let provider = MsSqlProvider { config: cfg.clone() };
        let db = cfg.profile.database.clone().unwrap_or_else(|| "master".into());
        introspect::run(
            &provider,
            &db,
            &introspect::information_schema_columns(""),
            Some(&introspect::information_schema_keys("")),
            Some(&introspect::information_schema_routines("")),
        )
        .await
    }

    fn provider(&self, config: ConnectionConfig) -> ConnectorResult<Arc<dyn DataProvider>> {
        Ok(Arc::new(MsSqlProvider { config }))
    }
}

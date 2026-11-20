//! Ponte universal ODBC (Tier 2): Oracle, IBM DB2, SAP HANA, Sybase, Firebird, Access/Excel...
//! Requer um Driver Manager no sistema (Windows `odbc32.dll`, Linux unixODBC, macOS iODBC/unixODBC).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use async_trait::async_trait;
use nova_domain::ParamValue;
use nova_domain::connection::{ColumnMetadata, ConnectionConfig, DatabaseTree, TableKind, TableMetadata};
use nova_domain::report::ProviderKind;
use nova_engine::EngineResult;
use nova_engine::data::{CellValue, DataProvider};
use odbc_api::buffers::TextRowSet;
use odbc_api::handles::DataType;
use odbc_api::parameter::InputParameter;
use odbc_api::{ConnectionOptions, Cursor, Environment, IntoParameter, ResultSetMetadata};
use polars::prelude::DataFrame;

use crate::params::{PlaceholderStyle, bind_named};
use crate::registry::{Driver, DriverInfo};
use crate::rowset::RowSet;
use crate::{ConnectorError, ConnectorResult};

const BATCH: usize = 2_000;
const MAX_TEXT: usize = 8_192;

static ENV: OnceLock<Result<Environment, String>> = OnceLock::new();

fn env() -> ConnectorResult<&'static Environment> {
    ENV.get_or_init(|| Environment::new().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| ConnectorError::Connection(format!("Driver Manager ODBC indisponível: {e}")))
}

fn connection_string(cfg: &ConnectionConfig) -> ConnectorResult<String> {
    if let Some(cs) = cfg.resolved_connection_string() {
        return Ok(cs);
    }
    let p = &cfg.profile;
    let dsn = p.database.clone().ok_or_else(|| ConnectorError::Config("informe a connection string ou o DSN".into()))?;
    let mut cs = format!("DSN={dsn};");
    if let Some(u) = &p.username {
        cs.push_str(&format!("UID={u};"));
    }
    if let Some(pw) = &cfg.password {
        cs.push_str(&format!("PWD={{{}}};", pw.replace('}', "}}")));
    }
    Ok(cs)
}

#[derive(Clone, Copy)]
enum Kind {
    Int,
    Float,
    Bool,
    Text,
}

fn kind_of(dt: &DataType) -> (Kind, &'static str) {
    match dt {
        DataType::Integer | DataType::SmallInt | DataType::BigInt | DataType::TinyInt => (Kind::Int, "INTEGER"),
        DataType::Numeric { scale: 0, .. } | DataType::Decimal { scale: 0, .. } => (Kind::Int, "INTEGER"),
        DataType::Numeric { .. } | DataType::Decimal { .. } | DataType::Float { .. } | DataType::Real | DataType::Double => (Kind::Float, "DOUBLE"),
        DataType::Bit => (Kind::Bool, "BOOLEAN"),
        DataType::Date => (Kind::Text, "DATE"),
        DataType::Timestamp { .. } => (Kind::Text, "TIMESTAMP"),
        _ => (Kind::Text, "TEXT"),
    }
}

fn to_param(v: &ParamValue) -> Box<dyn InputParameter> {
    match v {
        ParamValue::Int(i) => Box::new(*i),
        ParamValue::Float(f) => Box::new(*f),
        ParamValue::Bool(b) => Box::new(*b as i64),
        ParamValue::Null => Box::new(Option::<String>::None.into_parameter()),
        other => Box::new(other.to_text().into_parameter()),
    }
}

fn query_blocking(cs: &str, sql: &str, params: &HashMap<String, ParamValue>) -> ConnectorResult<DataFrame> {
    let q = |e: odbc_api::Error| ConnectorError::Query(e.to_string());
    let conn = env()?.connect_with_connection_string(cs, ConnectionOptions::default()).map_err(|e| ConnectorError::Connection(e.to_string()))?;
    let (sql, values) = bind_named(sql, params, PlaceholderStyle::Question);
    let boxed: Vec<Box<dyn InputParameter>> = values.iter().map(to_param).collect();
    let Some(mut cursor) = conn.execute(&sql, &boxed[..], None).map_err(q)? else {
        return RowSet::default().into_dataframe();
    };
    let names: Vec<String> = cursor.column_names().map_err(q)?.collect::<Result<_, _>>().map_err(q)?;
    let mut kinds = Vec::with_capacity(names.len());
    for i in 1..=names.len() as u16 {
        kinds.push(kind_of(&cursor.col_data_type(i).map_err(q)?));
    }
    let mut rs = RowSet::new(names.into_iter().zip(&kinds).map(|(n, (_, native))| (n, Some(native.to_string()))).collect());
    let buffer = TextRowSet::for_cursor(BATCH, &mut cursor, Some(MAX_TEXT)).map_err(q)?;
    let mut block = cursor.bind_buffer(buffer).map_err(q)?;
    while let Some(batch) = block.fetch().map_err(q)? {
        for r in 0..batch.num_rows() {
            let row = (0..batch.num_cols())
                .map(|c| match batch.at(c, r) {
                    None => CellValue::Null,
                    Some(bytes) => {
                        let s = String::from_utf8_lossy(bytes).trim().to_string();
                        match kinds[c].0 {
                            Kind::Int => s.parse().map(CellValue::Int).unwrap_or(CellValue::Text(s)),
                            Kind::Float => s.replace(',', ".").parse().map(CellValue::Float).unwrap_or(CellValue::Text(s)),
                            Kind::Bool => CellValue::Bool(s == "1" || s.eq_ignore_ascii_case("true")),
                            Kind::Text => CellValue::Text(s),
                        }
                    }
                })
                .collect();
            rs.rows.push(row);
        }
    }
    rs.into_dataframe()
}

pub struct OdbcProvider {
    connection_string: String,
}

#[async_trait]
impl DataProvider for OdbcProvider {
    async fn execute_query(&self, query: &str, params: &HashMap<String, ParamValue>) -> EngineResult<DataFrame> {
        let (cs, sql, params) = (self.connection_string.clone(), query.to_string(), params.clone());
        tokio::task::spawn_blocking(move || query_blocking(&cs, &sql, &params))
            .await
            .map_err(|e| nova_engine::EngineError::DataSource(e.to_string()))?
            .map_err(Into::into)
    }
}

pub struct OdbcDriver;

#[async_trait]
impl Driver for OdbcDriver {
    fn info(&self) -> DriverInfo {
        DriverInfo {
            id: "odbc",
            name: "ODBC (Oracle, DB2, HANA, Firebird...)",
            tier: "2-ponte",
            default_port: None,
            fields: &["connectionString", "database", "username", "password"],
            supports_introspection: true,
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Odbc
    }

    async fn test_connection(&self, cfg: &ConnectionConfig) -> ConnectorResult<u64> {
        let cs = connection_string(cfg)?;
        let start = Instant::now();
        tokio::task::spawn_blocking(move || {
            env()?.connect_with_connection_string(&cs, ConnectionOptions::default()).map(|_| ()).map_err(|e| ConnectorError::Connection(e.to_string()))
        })
        .await
        .map_err(|e| ConnectorError::Connection(e.to_string()))??;
        Ok(super::elapsed_ms(start))
    }

    /// Usa `SQLTables`/`SQLColumns` do Driver Manager.
    async fn introspect(&self, cfg: &ConnectionConfig) -> ConnectorResult<DatabaseTree> {
        let cs = connection_string(cfg)?;
        let db = cfg.profile.database.clone().unwrap_or_else(|| "odbc".into());
        tokio::task::spawn_blocking(move || -> ConnectorResult<DatabaseTree> {
            let q = |e: odbc_api::Error| ConnectorError::Query(e.to_string());
            let conn = env()?.connect_with_connection_string(&cs, ConnectionOptions::default()).map_err(|e| ConnectorError::Connection(e.to_string()))?;
            let mut cursor = conn.columns("", "", "", "").map_err(q)?;
            let buffer = TextRowSet::for_cursor(BATCH, &mut cursor, Some(512)).map_err(q)?;
            let mut block = cursor.bind_buffer(buffer).map_err(q)?;
            let mut tables: Vec<TableMetadata> = Vec::new();
            let txt = |b: Option<&[u8]>| b.map(|x| String::from_utf8_lossy(x).trim().to_string()).unwrap_or_default();
            while let Some(batch) = block.fetch().map_err(q)? {
                for r in 0..batch.num_rows() {
                    // colunas ODBC: 2=schema, 3=tabela, 4=coluna, 6=tipo, 11=nullable
                    let (s, t, c, ty) = (txt(batch.at(1, r)), txt(batch.at(2, r)), txt(batch.at(3, r)), txt(batch.at(5, r)));
                    if tables.last().is_none_or(|x| x.schema != s || x.name != t) {
                        tables.push(TableMetadata { schema: s.clone(), name: t.clone(), kind: TableKind::Table, comment: None, columns: vec![] });
                    }
                    tables.last_mut().expect("tabela").columns.push(ColumnMetadata {
                        arrow_type: crate::introspect::arrow_type(&ty).into(),
                        native_type: ty,
                        nullable: txt(batch.at(10, r)) != "0",
                        is_primary_key: false,
                        is_foreign_key: false,
                        default_value: None,
                        name: c,
                    });
                }
            }
            Ok(DatabaseTree::build(db, tables, vec![]))
        })
        .await
        .map_err(|e| ConnectorError::Connection(e.to_string()))?
    }

    fn provider(&self, config: ConnectionConfig) -> ConnectorResult<Arc<dyn DataProvider>> {
        Ok(Arc::new(OdbcProvider { connection_string: connection_string(&config)? }))
    }
}

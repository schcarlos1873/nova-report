//! Utilitários compartilhados pelos drivers baseados em `sqlx`.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use nova_engine::data::CellValue;
use rust_decimal::prelude::ToPrimitive;
use sqlx::{Column, Row, TypeInfo, ValueRef};
use tokio::sync::Mutex;

/// Cache de pools por fingerprint da conexão (preview rápido sem reconectar).
pub(crate) type PoolCache<P> = Arc<Mutex<HashMap<String, P>>>;

/// Decodifica uma célula pelo nome do tipo nativo, com fallback textual.
pub(crate) fn decode_cell<R>(row: &R, i: usize) -> CellValue
where
    R: Row,
    usize: sqlx::ColumnIndex<R>,
    for<'a> i64: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> i32: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> i16: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> f64: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> f32: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> bool: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> String: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> Vec<u8>: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> NaiveDate: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> NaiveDateTime: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> DateTime<Utc>: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> rust_decimal::Decimal: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
{
    match row.try_get_raw(i) {
        Ok(raw) if raw.is_null() => return CellValue::Null,
        Err(_) => return CellValue::Null,
        _ => {}
    }
    let ty = row.columns()[i].type_info().name().to_ascii_uppercase();
    macro_rules! get {
        ($t:ty) => {
            row.try_get::<$t, _>(i).ok()
        };
    }
    let typed = match ty.as_str() {
        "BOOL" | "BOOLEAN" => get!(bool).map(CellValue::Bool),
        "INT8" | "BIGINT" => get!(i64).map(CellValue::Int),
        "INT4" | "INT" | "INTEGER" | "MEDIUMINT" => get!(i32).map(|v| CellValue::Int(v as i64)),
        "INT2" | "SMALLINT" => get!(i16).map(|v| CellValue::Int(v as i64)),
        "FLOAT8" | "DOUBLE" => get!(f64).map(CellValue::Float),
        "FLOAT4" | "FLOAT" | "REAL" => get!(f32).map(|v| CellValue::Float(v as f64)),
        "NUMERIC" | "DECIMAL" => get!(rust_decimal::Decimal).and_then(|d| d.to_f64()).map(CellValue::Float),
        "DATE" => get!(NaiveDate).map(CellValue::Date),
        "TIMESTAMP" | "DATETIME" => get!(NaiveDateTime)
            .or_else(|| get!(DateTime<Utc>).map(|d| d.naive_utc()))
            .map(CellValue::DateTime),
        "TIMESTAMPTZ" => get!(DateTime<Utc>).map(|d| CellValue::DateTime(d.naive_utc())),
        t if t.contains("UNSIGNED") || t == "TINYINT" => get!(i64).map(CellValue::Int),
        _ => None,
    };
    typed
        .or_else(|| row.try_get::<String, _>(i).ok().map(CellValue::Text))
        .or_else(|| row.try_get_unchecked::<String, _>(i).ok().map(CellValue::Text))
        .or_else(|| row.try_get_unchecked::<Vec<u8>, _>(i).ok().map(|b| CellValue::Text(String::from_utf8_lossy(&b).into_owned())))
        .unwrap_or(CellValue::Null)
}

/// Faz bind de `ParamValue` em uma query sqlx (macro para evitar bounds genéricos extensos).
macro_rules! bind_all {
    ($query:expr, $values:expr) => {{
        let mut q = $query;
        for v in $values {
            q = match v {
                nova_domain::ParamValue::Int(i) => q.bind(i),
                nova_domain::ParamValue::Float(f) => q.bind(f),
                nova_domain::ParamValue::Bool(b) => q.bind(b),
                nova_domain::ParamValue::Text(s) => q.bind(s),
                nova_domain::ParamValue::Date(d) => q.bind(d),
                nova_domain::ParamValue::DateTime(d) => q.bind(d),
                nova_domain::ParamValue::Null => q.bind(Option::<String>::None),
            };
        }
        q
    }};
}
pub(crate) use bind_all;

/// Implementa `DataProvider` sobre um pool sqlx.
macro_rules! sqlx_provider {
    ($provider:ident, $pool:ty, $style:expr) => {
        pub struct $provider {
            pub(crate) pool: $pool,
        }

        #[async_trait::async_trait]
        impl nova_engine::data::DataProvider for $provider {
            async fn execute_query(
                &self,
                query: &str,
                params: &std::collections::HashMap<String, nova_domain::ParamValue>,
            ) -> nova_engine::EngineResult<polars::prelude::DataFrame> {
                use sqlx::{Column, Executor, Row};
                let (sql, values) = crate::params::bind_named(query, params, $style);
                let q = crate::drivers::sqlx_common::bind_all!(sqlx::query(&sql), values);
                let rows = q
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| nova_engine::EngineError::DataSource(e.to_string()))?;
                let columns: Vec<(String, Option<String>)> = match rows.first() {
                    Some(r) => r
                        .columns()
                        .iter()
                        .map(|c| (c.name().to_string(), Some(sqlx::TypeInfo::name(c.type_info()).to_string())))
                        .collect(),
                    // sem linhas: usa describe para preservar o schema
                    None => match self.pool.describe(&sql).await {
                        Ok(d) => d
                            .columns()
                            .iter()
                            .map(|c| (c.name().to_string(), Some(sqlx::TypeInfo::name(c.type_info()).to_string())))
                            .collect(),
                        Err(_) => vec![],
                    },
                };
                let mut rs = crate::rowset::RowSet::new(columns);
                for row in &rows {
                    rs.rows.push((0..row.len()).map(|i| crate::drivers::sqlx_common::decode_cell(row, i)).collect());
                }
                rs.into_dataframe().map_err(Into::into)
            }
        }
    };
}
pub(crate) use sqlx_provider;

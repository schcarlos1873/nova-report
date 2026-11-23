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

/// Sonda TCP antes de criar o pool (o sqlx tenta os endereços em sequência, sem Happy
/// Eyeballs): devolve erro diagnosticado e, quando o DNS publica endereços inválidos e o
/// hostname não precisa ser verificado pelo TLS, o IP que respondeu para fixar no pool.
pub(crate) async fn probe(
    host: &str,
    port: u16,
    is_socket: bool,
    verifies_hostname: bool,
    timeout: std::time::Duration,
) -> crate::ConnectorResult<Option<std::net::IpAddr>> {
    if is_socket || host.starts_with('/') {
        return Ok(None);
    }
    let deadline = tokio::time::Instant::now() + timeout;
    let c = crate::net::connect_tcp(host, port, deadline, timeout).await?;
    let literal = host.trim_matches(['[', ']']).parse::<std::net::IpAddr>().is_ok();
    let pin = c.dns_had_invalid && !literal && !verifies_hostname;
    if pin {
        crate::diag::warn(format!("DNS com endereços inválidos: o pool usará o IP {} diretamente", c.addr.ip()));
    }
    Ok(pin.then(|| c.addr.ip()))
}

/// Traduz erros do sqlx em mensagens com dica. `auth_codes`/`db_codes`: códigos do servidor.
pub(crate) fn diagnose(
    e: sqlx::Error,
    host: &str,
    port: u16,
    timeout: std::time::Duration,
    auth_codes: &[&str],
    db_codes: &[&str],
) -> crate::ConnectorError {
    use crate::{ConnectorError, NetErrorKind};
    let err = match e {
        sqlx::Error::PoolTimedOut => crate::net::timeout_error("autenticar (TLS/login)", host, port, timeout, None),
        sqlx::Error::Tls(err) => ConnectorError::network(
            NetErrorKind::Tls,
            format!("Falha no handshake TLS com {host}:{port}."),
            Some(format!("confira o modo TLS/certificado do servidor. ({err})")),
        ),
        sqlx::Error::Database(db) => {
            let code = db.code().map(|c| c.to_string()).unwrap_or_default();
            if auth_codes.contains(&code.as_str()) {
                ConnectorError::network(NetErrorKind::Auth, "Usuário ou senha inválidos.", Some(db.message().to_string()))
            } else if db_codes.contains(&code.as_str()) {
                ConnectorError::network(
                    NetErrorKind::Database,
                    "Banco de dados inexistente ou sem permissão.",
                    Some(db.message().to_string()),
                )
            } else {
                ConnectorError::Connection(db.to_string())
            }
        }
        other => ConnectorError::Connection(other.to_string()),
    };
    crate::diag::error(err.to_string());
    err
}

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
        .or_else(|| row.try_get_unchecked::<Vec<u8>, _>(i).ok().map(|b| match String::from_utf8(b) { Ok(s) => CellValue::Text(s), Err(e) => crate::rowset::binary_cell(e.as_bytes()) }))
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
                // `bind_named` já expandiu listas em escalares; fallback defensivo
                nova_domain::ParamValue::List(_) => q.bind(v.to_text()),
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

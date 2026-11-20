//! Arquivos locais (CSV / JSON). O texto da query pode ser vazio (arquivo inteiro) ou
//! um SQL executado em memória pelo Polars sobre a tabela `data`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use nova_domain::ParamValue;
use nova_domain::connection::{ColumnMetadata, ConnectionConfig, DatabaseTree, TableKind, TableMetadata};
use nova_domain::report::ProviderKind;
use nova_engine::EngineResult;
use nova_engine::data::{CellValue, DataProvider};
use polars::prelude::*;

use crate::params::inline_literals;
use crate::registry::{Driver, DriverInfo};
use crate::rowset::RowSet;
use crate::{ConnectorError, ConnectorResult};

#[derive(Debug, Clone, Copy)]
enum Format {
    Csv,
    Json,
}

fn file_path(config: &ConnectionConfig) -> ConnectorResult<PathBuf> {
    config
        .profile
        .path
        .clone()
        .map(PathBuf::from)
        .ok_or_else(|| ConnectorError::Config("informe o caminho do arquivo".into()))
}

fn read_file(path: &PathBuf, format: Format, options: &std::collections::BTreeMap<String, String>) -> ConnectorResult<DataFrame> {
    if !path.exists() {
        return Err(ConnectorError::Connection(format!("arquivo não encontrado: {}", path.display())));
    }
    match format {
        Format::Csv => {
            let sep = options.get("separator").and_then(|s| s.bytes().next()).unwrap_or(b',');
            Ok(CsvReadOptions::default()
                .with_has_header(options.get("header").is_none_or(|h| h != "false"))
                .with_infer_schema_length(Some(10_000))
                .with_parse_options(CsvParseOptions::default().with_separator(sep).with_try_parse_dates(true))
                .try_into_reader_with_file_path(Some(path.clone()))?
                .finish()?)
        }
        Format::Json => {
            let text = std::fs::read_to_string(path)?;
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| ConnectorError::Query(format!("JSON inválido: {e}")))?;
            json_to_rowset(&value, options.get("jsonPath").map(String::as_str))?.into_dataframe()
        }
    }
}

/// Aplica SQL em memória (Polars SQL) sobre o dataset registrado como `data`.
pub(crate) fn apply_sql(df: DataFrame, sql: &str, params: &HashMap<String, ParamValue>) -> ConnectorResult<DataFrame> {
    let trimmed = sql.trim();
    if trimmed.is_empty() || trimmed == "*" || !trimmed.to_ascii_lowercase().starts_with("select") && !trimmed.to_ascii_lowercase().starts_with("with") {
        return Ok(df);
    }
    let mut ctx = polars::sql::SQLContext::new();
    ctx.register("data", df.lazy());
    Ok(ctx.execute(&inline_literals(trimmed, params))?.collect()?)
}

/// Converte JSON (array de objetos, ou objeto com array em `data`/`items`/`results`/`rows`).
pub(crate) fn json_to_rowset(value: &serde_json::Value, json_path: Option<&str>) -> ConnectorResult<RowSet> {
    use serde_json::Value as J;
    let mut node = value;
    if let Some(path) = json_path.filter(|p| !p.is_empty()) {
        for key in path.split('.') {
            node = node.get(key).ok_or_else(|| ConnectorError::Query(format!("caminho JSON '{path}' não encontrado")))?;
        }
    } else if let J::Object(map) = node {
        if let Some(arr) = ["data", "items", "results", "rows", "records"].iter().find_map(|k| map.get(*k).filter(|v| v.is_array())) {
            node = arr;
        }
    }
    let items: Vec<&J> = match node {
        J::Array(a) => a.iter().collect(),
        other => vec![other],
    };
    let mut names: Vec<String> = Vec::new();
    for it in &items {
        if let J::Object(m) = it {
            for k in m.keys() {
                if !names.contains(k) {
                    names.push(k.clone());
                }
            }
        }
    }
    if names.is_empty() {
        names.push("value".into());
    }
    let mut rs = RowSet::new(names.iter().map(|n| (n.clone(), None)).collect());
    for it in items {
        let row = names
            .iter()
            .map(|n| {
                let v = match it {
                    J::Object(m) => m.get(n).unwrap_or(&J::Null),
                    other if n == "value" => other,
                    _ => &J::Null,
                };
                match v {
                    J::Null => CellValue::Null,
                    J::Bool(b) => CellValue::Bool(*b),
                    J::Number(x) => x.as_i64().map(CellValue::Int).unwrap_or_else(|| CellValue::Float(x.as_f64().unwrap_or(0.0))),
                    J::String(s) => CellValue::Text(s.clone()),
                    other => CellValue::Text(other.to_string()),
                }
            })
            .collect();
        rs.rows.push(row);
    }
    Ok(rs)
}

pub struct FileProvider {
    path: PathBuf,
    format: Format,
    options: std::collections::BTreeMap<String, String>,
}

#[async_trait]
impl DataProvider for FileProvider {
    async fn execute_query(&self, query: &str, params: &HashMap<String, ParamValue>) -> EngineResult<DataFrame> {
        let (path, format, options, sql, params) = (self.path.clone(), self.format, self.options.clone(), query.to_string(), params.clone());
        tokio::task::spawn_blocking(move || {
            let df = read_file(&path, format, &options)?;
            apply_sql(df, &sql, &params)
        })
        .await
        .map_err(|e| nova_engine::EngineError::DataSource(e.to_string()))?
        .map_err(Into::into)
    }
}

async fn test_file(config: &ConnectionConfig, format: Format) -> ConnectorResult<u64> {
    let start = Instant::now();
    let path = file_path(config)?;
    let options = config.profile.options.clone();
    tokio::task::spawn_blocking(move || read_file(&path, format, &options).map(|_| ()))
        .await
        .map_err(|e| ConnectorError::Connection(e.to_string()))??;
    Ok(super::elapsed_ms(start))
}

async fn introspect_file(config: &ConnectionConfig, format: Format) -> ConnectorResult<DatabaseTree> {
    let path = file_path(config)?;
    let options = config.profile.options.clone();
    let p = path.clone();
    let df = tokio::task::spawn_blocking(move || read_file(&p, format, &options))
        .await
        .map_err(|e| ConnectorError::Connection(e.to_string()))??;
    let columns = df
        .columns()
        .iter()
        .map(|c| ColumnMetadata {
            name: c.name().to_string(),
            native_type: c.dtype().to_string(),
            arrow_type: format!("{:?}", c.dtype()),
            nullable: true,
            is_primary_key: false,
            is_foreign_key: false,
            default_value: None,
        })
        .collect();
    let name = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(DatabaseTree::build(
        name,
        vec![TableMetadata { schema: "arquivo".into(), name: "data".into(), kind: TableKind::Table, comment: None, columns }],
        vec![],
    ))
}

macro_rules! file_driver {
    ($name:ident, $kind:expr, $fmt:expr, $id:literal, $label:literal) => {
        pub struct $name;

        #[async_trait]
        impl Driver for $name {
            fn info(&self) -> DriverInfo {
                DriverInfo { id: $id, name: $label, tier: "arquivo", default_port: None, fields: &["path"], supports_introspection: true }
            }
            fn kind(&self) -> ProviderKind {
                $kind
            }
            async fn test_connection(&self, config: &ConnectionConfig) -> ConnectorResult<u64> {
                test_file(config, $fmt).await
            }
            async fn introspect(&self, config: &ConnectionConfig) -> ConnectorResult<DatabaseTree> {
                introspect_file(config, $fmt).await
            }
            fn provider(&self, config: ConnectionConfig) -> ConnectorResult<Arc<dyn DataProvider>> {
                Ok(Arc::new(FileProvider { path: file_path(&config)?, format: $fmt, options: config.profile.options.clone() }))
            }
        }
    };
}

file_driver!(CsvDriver, ProviderKind::Csv, Format::Csv, "csv", "Arquivo CSV");
file_driver!(JsonDriver, ProviderKind::Json, Format::Json, "json", "Arquivo JSON");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_arrays_nested_under_data() {
        let v = serde_json::json!({ "data": [ {"a": 1, "b": "x"}, {"a": 2.5, "c": true} ] });
        let df = json_to_rowset(&v, None).unwrap().into_dataframe().unwrap();
        assert_eq!(df.shape(), (2, 3));
        assert_eq!(df.column("a").unwrap().dtype(), &DataType::Float64);
    }

    #[test]
    fn sql_over_in_memory_frame() {
        let v = serde_json::json!([ {"cli": "a", "v": 1}, {"cli": "b", "v": 5}, {"cli": "a", "v": 2} ]);
        let df = json_to_rowset(&v, None).unwrap().into_dataframe().unwrap();
        let params = HashMap::from([("min".to_string(), ParamValue::Int(1))]);
        let out = apply_sql(df, "SELECT cli, SUM(v) AS total FROM data WHERE v > :min GROUP BY cli ORDER BY cli", &params).unwrap();
        assert_eq!(out.height(), 2);
    }
}

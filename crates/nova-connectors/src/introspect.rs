//! Introspecção de schema (estilo DBeaver) construída a partir de queries de catálogo
//! executadas pelo próprio `DataProvider` de cada driver.

use std::collections::{HashMap, HashSet};

use nova_domain::connection::{ColumnMetadata, DatabaseTree, ProcedureMetadata, TableKind, TableMetadata};
use nova_engine::data::{CellValue, DataProvider};
use polars::prelude::DataFrame;

use crate::ConnectorResult;

const SYSTEM_SCHEMAS: &str = "'pg_catalog','information_schema','pg_toast','sys','mysql','performance_schema','INFORMATION_SCHEMA'";

/// Catálogo ANSI (PostgreSQL, MySQL/MariaDB, SQL Server).
pub(crate) fn information_schema_columns(extra_filter: &str) -> String {
    format!(
        "SELECT c.TABLE_SCHEMA AS s, c.TABLE_NAME AS t, tb.TABLE_TYPE AS k, c.COLUMN_NAME AS c, \
         c.DATA_TYPE AS dt, c.IS_NULLABLE AS n, c.COLUMN_DEFAULT AS d \
         FROM INFORMATION_SCHEMA.COLUMNS c \
         JOIN INFORMATION_SCHEMA.TABLES tb ON tb.TABLE_SCHEMA = c.TABLE_SCHEMA AND tb.TABLE_NAME = c.TABLE_NAME \
         WHERE c.TABLE_SCHEMA NOT IN ({SYSTEM_SCHEMAS}) {extra_filter} \
         ORDER BY c.TABLE_SCHEMA, c.TABLE_NAME, c.ORDINAL_POSITION"
    )
}

pub(crate) fn information_schema_keys(extra_filter: &str) -> String {
    format!(
        "SELECT k.TABLE_SCHEMA AS s, k.TABLE_NAME AS t, k.COLUMN_NAME AS c, tc.CONSTRAINT_TYPE AS ct \
         FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc \
         JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE k ON k.CONSTRAINT_NAME = tc.CONSTRAINT_NAME \
           AND k.TABLE_SCHEMA = tc.TABLE_SCHEMA AND k.TABLE_NAME = tc.TABLE_NAME \
         WHERE tc.CONSTRAINT_TYPE IN ('PRIMARY KEY', 'FOREIGN KEY') {}",
        extra_filter.replace("c.TABLE_SCHEMA", "k.TABLE_SCHEMA")
    )
}

pub(crate) fn information_schema_routines(extra_filter: &str) -> String {
    format!(
        "SELECT ROUTINE_SCHEMA AS s, ROUTINE_NAME AS t FROM INFORMATION_SCHEMA.ROUTINES \
         WHERE ROUTINE_SCHEMA NOT IN ({SYSTEM_SCHEMAS}) {} ORDER BY ROUTINE_SCHEMA, ROUTINE_NAME",
        extra_filter.replace("c.TABLE_SCHEMA", "ROUTINE_SCHEMA")
    )
}

pub(crate) const SQLITE_CATALOG: &str = "SELECT 'main' AS s, m.name AS t, m.type AS k, p.name AS c, p.type AS dt, \
     CASE WHEN p.\"notnull\" = 1 THEN 'NO' ELSE 'YES' END AS n, p.dflt_value AS d, p.pk AS pk \
     FROM sqlite_master m JOIN pragma_table_info(m.name) p \
     WHERE m.type IN ('table', 'view') AND m.name NOT LIKE 'sqlite_%' ORDER BY m.name, p.cid";

pub(crate) const SQLITE_FOREIGN_KEYS: &str = "SELECT 'main' AS s, m.name AS t, f.\"from\" AS c, 'FOREIGN KEY' AS ct \
     FROM sqlite_master m JOIN pragma_foreign_key_list(m.name) f WHERE m.type = 'table'";

/// Executa as queries de catálogo e monta a árvore. Falhas nas queries opcionais
/// (chaves, rotinas) não impedem a navegação.
pub(crate) async fn run(
    provider: &dyn DataProvider,
    database_name: &str,
    columns_sql: &str,
    keys_sql: Option<&str>,
    routines_sql: Option<&str>,
) -> ConnectorResult<DatabaseTree> {
    let empty = HashMap::new();
    let columns = provider.execute_query(columns_sql, &empty).await?;
    let keys = match keys_sql {
        Some(q) => provider.execute_query(q, &empty).await.ok(),
        None => None,
    };
    let routines = match routines_sql {
        Some(q) => provider.execute_query(q, &empty).await.ok(),
        None => None,
    };
    Ok(build(database_name, &columns, keys.as_ref(), routines.as_ref()))
}

fn text(df: &DataFrame, col: &str, row: usize) -> String {
    df.get_column_names()
        .into_iter()
        .find(|n| n.eq_ignore_ascii_case(col))
        .and_then(|n| df.column(n.as_str()).ok())
        .and_then(|c| c.get(row).ok())
        .map(|v| CellValue::from_any(&v).to_display())
        .unwrap_or_default()
}

pub(crate) fn arrow_type(native: &str) -> &'static str {
    let n = native.to_ascii_lowercase();
    if n == "date" {
        "Date32"
    } else if n.contains("timestamp") || n.contains("datetime") {
        "Timestamp(ms)"
    } else if n.contains("bool") || n == "bit" {
        "Boolean"
    } else if n.contains("int") || n.contains("serial") {
        "Int64"
    } else if ["numeric", "decimal", "real", "double", "float", "money"].iter().any(|k| n.contains(k)) {
        "Float64"
    } else if n.contains("blob") || n.contains("binary") || n == "bytea" {
        "Binary"
    } else {
        "Utf8"
    }
}

fn build(db: &str, columns: &DataFrame, keys: Option<&DataFrame>, routines: Option<&DataFrame>) -> DatabaseTree {
    let mut pks = HashSet::new();
    let mut fks = HashSet::new();
    if let Some(k) = keys {
        for r in 0..k.height() {
            let key = (text(k, "s", r), text(k, "t", r), text(k, "c", r));
            if text(k, "ct", r).starts_with("PRIMARY") { pks.insert(key) } else { fks.insert(key) };
        }
    }
    let has_pk_col = columns.get_column_names().iter().any(|n| n.as_str() == "pk");
    let mut tables: Vec<TableMetadata> = Vec::new();
    for r in 0..columns.height() {
        let (s, t, c) = (text(columns, "s", r), text(columns, "t", r), text(columns, "c", r));
        let kind = if text(columns, "k", r).to_ascii_uppercase().contains("VIEW") { TableKind::View } else { TableKind::Table };
        if tables.last().is_none_or(|x| x.schema != s || x.name != t) {
            tables.push(TableMetadata { schema: s.clone(), name: t.clone(), kind, comment: None, columns: vec![] });
        }
        let native = text(columns, "dt", r);
        let is_pk = pks.contains(&(s.clone(), t.clone(), c.clone()))
            || (has_pk_col && text(columns, "pk", r).parse::<i64>().unwrap_or(0) > 0);
        let default = text(columns, "d", r);
        tables.last_mut().expect("tabela inserida").columns.push(ColumnMetadata {
            arrow_type: arrow_type(&native).into(),
            native_type: native,
            nullable: text(columns, "n", r).eq_ignore_ascii_case("YES"),
            is_primary_key: is_pk,
            is_foreign_key: fks.contains(&(s, t, c.clone())),
            default_value: (!default.is_empty()).then_some(default),
            name: c,
        });
    }
    let procedures = routines
        .map(|df| {
            (0..df.height())
                .map(|r| ProcedureMetadata { schema: text(df, "s", r), name: text(df, "t", r), parameters: vec![] })
                .collect()
        })
        .unwrap_or_default();
    DatabaseTree::build(db, tables, procedures)
}

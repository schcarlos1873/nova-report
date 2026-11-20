use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTree {
    pub database_name: String,
    pub schemas: Vec<SchemaMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaMetadata {
    pub name: String,
    pub tables: Vec<TableMetadata>,
    pub views: Vec<TableMetadata>,
    pub procedures: Vec<ProcedureMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TableKind {
    Table,
    View,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMetadata {
    pub schema: String,
    pub name: String,
    pub kind: TableKind,
    pub comment: Option<String>,
    pub columns: Vec<ColumnMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMetadata {
    pub name: String,
    pub native_type: String,
    /// Tipo normalizado para Apache Arrow.
    pub arrow_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcedureMetadata {
    pub schema: String,
    pub name: String,
    pub parameters: Vec<ProcedureParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcedureParam {
    pub name: String,
    pub data_type: String,
    /// `IN`, `OUT` ou `INOUT`.
    pub direction: String,
}

impl DatabaseTree {
    /// Agrupa uma lista plana de tabelas/views em schemas.
    pub fn build(database_name: impl Into<String>, tables: Vec<TableMetadata>, procedures: Vec<ProcedureMetadata>) -> Self {
        use std::collections::BTreeMap;
        let mut schemas: BTreeMap<String, SchemaMetadata> = BTreeMap::new();
        let entry = |m: &mut BTreeMap<String, SchemaMetadata>, s: &str| {
            m.entry(s.to_string()).or_insert_with(|| SchemaMetadata {
                name: s.to_string(),
                tables: vec![],
                views: vec![],
                procedures: vec![],
            });
        };
        for t in tables {
            entry(&mut schemas, &t.schema);
            let s = schemas.get_mut(&t.schema).expect("schema inserido");
            match t.kind {
                TableKind::Table => s.tables.push(t),
                TableKind::View => s.views.push(t),
            }
        }
        for p in procedures {
            entry(&mut schemas, &p.schema);
            schemas.get_mut(&p.schema).expect("schema inserido").procedures.push(p);
        }
        Self { database_name: database_name.into(), schemas: schemas.into_values().collect() }
    }
}

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Sqlite,
    Postgres,
    Mysql,
    Mssql,
    Odbc,
    Csv,
    Json,
    Rest,
    Grpc,
}

impl ProviderKind {
    pub fn id(self) -> &'static str {
        match self {
            ProviderKind::Sqlite => "sqlite",
            ProviderKind::Postgres => "postgres",
            ProviderKind::Mysql => "mysql",
            ProviderKind::Mssql => "mssql",
            ProviderKind::Odbc => "odbc",
            ProviderKind::Csv => "csv",
            ProviderKind::Json => "json",
            ProviderKind::Rest => "rest",
            ProviderKind::Grpc => "grpc",
        }
    }

    pub fn is_sql(self) -> bool {
        matches!(
            self,
            ProviderKind::Sqlite
                | ProviderKind::Postgres
                | ProviderKind::Mysql
                | ProviderKind::Mssql
                | ProviderKind::Odbc
        )
    }
}

/// Fonte de dados declarada no `.nrep`. **Nunca** contém senhas: credenciais são
/// resolvidas em tempo de execução via `connectionRef` (perfil + cofre do SO) ou
/// `connectionStringRef` (variável de ambiente).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceDef {
    pub id: String,
    pub provider: ProviderKind,
    /// Alias de um perfil de conexão salvo (ex.: `ERP_PROD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_ref: Option<String>,
    /// Nome de variável de ambiente contendo a connection string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_string_ref: Option<String>,
    /// Opções não sensíveis (ex.: `path` de um arquivo SQLite/CSV, `url` REST).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
    #[serde(default)]
    pub queries: Vec<QueryDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryDef {
    pub name: String,
    /// SQL com parâmetros nomeados (`:nome`) ou, para REST/arquivos, o endpoint/caminho.
    pub sql: String,
}

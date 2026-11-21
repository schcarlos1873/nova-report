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
    /// Vazio quando `kind` é `storedProcedure`.
    #[serde(default)]
    pub sql: String,
    #[serde(default, skip_serializing_if = "QueryKind::is_text")]
    pub kind: QueryKind,
    /// Nome da stored procedure (pode incluir o schema: `dbo.get_new_orders`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub procedure: Option<String>,
    /// Mapeamento dos parâmetros da procedure, na ordem de chamada.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub procedure_params: Vec<ProcedureParam>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueryKind {
    #[default]
    Text,
    StoredProcedure,
}

impl QueryKind {
    pub fn is_text(&self) -> bool {
        *self == QueryKind::Text
    }
}

/// Um argumento da stored procedure: `@param1` recebe `:param1` (parâmetro do relatório) ou um literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcedureParam {
    /// Nome do parâmetro na procedure (`@param1`; o prefixo `@` é opcional).
    pub name: String,
    /// `:nome` de um parâmetro do relatório ou literal SQL (`'A'`, `42`).
    #[serde(default)]
    pub value: String,
    /// Envia `NULL` em vez do valor.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pass_null: bool,
}

impl QueryDef {
    pub fn text(name: impl Into<String>, sql: impl Into<String>) -> Self {
        Self { name: name.into(), sql: sql.into(), kind: QueryKind::Text, procedure: None, procedure_params: vec![] }
    }

    pub fn is_procedure(&self) -> bool {
        self.kind == QueryKind::StoredProcedure
    }

    /// SQL efetivamente enviada ao banco: o texto da query ou a chamada da procedure no
    /// dialeto do provider. `None` quando o provider não suporta procedures.
    pub fn effective_sql(&self, provider: ProviderKind) -> Option<String> {
        if !self.is_procedure() {
            return Some(self.sql.clone());
        }
        let proc_name = self.procedure.as_deref().unwrap_or("").trim();
        let bare = |n: &str| n.trim().trim_start_matches('@').to_string();
        let value = |p: &ProcedureParam| if p.pass_null || p.value.trim().is_empty() { "NULL".to_string() } else { p.value.trim().to_string() };
        let args = |f: &dyn Fn(&ProcedureParam) -> String| self.procedure_params.iter().map(f).collect::<Vec<_>>().join(", ");
        match provider {
            ProviderKind::Mssql => {
                let a = args(&|p| format!("@{} = {}", bare(&p.name), value(p)));
                Some(if a.is_empty() { format!("EXEC {proc_name}") } else { format!("EXEC {proc_name} {a}") })
            }
            ProviderKind::Postgres => Some(format!("SELECT * FROM {proc_name}({})", args(&|p| format!("{} => {}", bare(&p.name), value(p))))),
            ProviderKind::Mysql => Some(format!("CALL {proc_name}({})", args(&|p| value(p)))),
            _ => None,
        }
    }

    /// Texto onde os `:parâmetros` do relatório aparecem (SQL ou valores da procedure).
    pub fn parameter_text(&self) -> String {
        if self.is_procedure() {
            self.procedure_params.iter().filter(|p| !p.pass_null).map(|p| p.value.as_str()).collect::<Vec<_>>().join(" ")
        } else {
            self.sql.clone()
        }
    }
}

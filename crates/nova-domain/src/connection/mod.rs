//! Perfis de conexão e metadados de schema.

mod metadata;

pub use metadata::{
    ColumnMetadata, DatabaseTree, ProcedureMetadata, ProcedureParam, SchemaMetadata, TableKind,
    TableMetadata,
};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::report::ProviderKind;

/// Perfil persistido de uma conexão (sem segredos).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    /// Identificador usado pelo `.nrep` em `connectionRef`.
    pub alias: String,
    pub provider: ProviderKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Arquivo local (SQLite/CSV/JSON) ou URL (REST).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Connection string completa (ODBC/ADO). Não deve conter senha: use `{PASSWORD}` como marcador.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_string: Option<String>,
    #[serde(default)]
    pub use_tls: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

/// Configuração pronta para o driver: perfil + segredo resolvido em runtime.
#[derive(Clone)]
pub struct ConnectionConfig {
    pub profile: ConnectionProfile,
    pub password: Option<String>,
}

impl std::fmt::Debug for ConnectionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionConfig")
            .field("profile", &self.profile)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .finish()
    }
}

impl ConnectionConfig {
    pub fn new(profile: ConnectionProfile, password: Option<String>) -> Self {
        Self { profile, password }
    }

    /// Chave estável para cache de pools (inclui hash do segredo, nunca o segredo).
    pub fn fingerprint(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&self.profile).unwrap_or_default().hash(&mut h);
        self.password.hash(&mut h);
        format!("{}:{:x}", self.profile.alias, h.finish())
    }

    /// Connection string com o marcador `{PASSWORD}` substituído.
    pub fn resolved_connection_string(&self) -> Option<String> {
        self.profile
            .connection_string
            .as_ref()
            .map(|cs| cs.replace("{PASSWORD}", self.password.as_deref().unwrap_or("")))
    }
}

//! Resolve a fonte de dados declarada no `.nrep` para uma configuração concreta,
//! obtendo segredos do cofre — o `.nrep` nunca carrega senhas.

use std::sync::Arc;

use nova_domain::connection::{ConnectionConfig, ConnectionProfile};
use nova_domain::ports::{ConnectionProfileRepository, CredentialVault};
use nova_domain::report::DataSourceDef;
use nova_domain::DomainError;

use crate::{AppError, AppResult};

#[derive(Clone)]
pub struct ConnectionResolver {
    profiles: Arc<dyn ConnectionProfileRepository>,
    vault: Arc<dyn CredentialVault>,
}

impl ConnectionResolver {
    pub fn new(profiles: Arc<dyn ConnectionProfileRepository>, vault: Arc<dyn CredentialVault>) -> Self {
        Self { profiles, vault }
    }

    pub fn profiles(&self) -> &Arc<dyn ConnectionProfileRepository> {
        &self.profiles
    }

    pub fn vault(&self) -> &Arc<dyn CredentialVault> {
        &self.vault
    }

    /// Perfil salvo + segredo do cofre.
    pub fn by_alias(&self, alias: &str) -> AppResult<ConnectionConfig> {
        let profile = self.profiles.get(alias)?.ok_or_else(|| DomainError::ConnectionNotFound(alias.into()))?;
        let password = self.vault.get(alias)?;
        Ok(ConnectionConfig::new(profile, password))
    }

    /// Ordem de resolução: `connectionRef` → `connectionStringRef` (env) → `options` inline.
    /// Caminhos relativos são resolvidos a partir do diretório do `.nrep`.
    pub fn for_data_source(&self, ds: &DataSourceDef, base_dir: Option<&std::path::Path>) -> AppResult<ConnectionConfig> {
        let mut cfg = if let Some(alias) = &ds.connection_ref {
            let mut c = self.by_alias(alias)?;
            if c.profile.provider != ds.provider {
                return Err(AppError::Invalid(format!(
                    "fonte '{}' declara provider '{}' mas a conexão '{alias}' é '{}'",
                    ds.id,
                    ds.provider.id(),
                    c.profile.provider.id()
                )));
            }
            c.profile.options.extend(ds.options.clone());
            c
        } else if let Some(var) = &ds.connection_string_ref {
            let cs = std::env::var(var)
                .map_err(|_| AppError::Invalid(format!("variável de ambiente '{var}' não definida (fonte '{}')", ds.id)))?;
            let mut p = inline_profile(ds);
            if ds.provider.is_sql() && ds.provider != nova_domain::report::ProviderKind::Sqlite {
                p.connection_string = Some(cs);
            } else {
                p.path = Some(cs);
            }
            ConnectionConfig::new(p, None)
        } else {
            ConnectionConfig::new(inline_profile(ds), None)
        };
        if let (Some(base), Some(path)) = (base_dir, cfg.profile.path.clone()) {
            let p = std::path::Path::new(&path);
            if p.is_relative() && !path.contains("://") {
                cfg.profile.path = Some(base.join(p).to_string_lossy().into_owned());
            }
        }
        Ok(cfg)
    }
}

fn inline_profile(ds: &DataSourceDef) -> ConnectionProfile {
    let opt = |k: &str| ds.options.get(k).cloned();
    ConnectionProfile {
        alias: ds.id.clone(),
        provider: ds.provider,
        host: opt("host"),
        port: opt("port").and_then(|p| p.parse().ok()),
        database: opt("database"),
        username: opt("username"),
        path: opt("path").or_else(|| opt("url")),
        connection_string: None,
        use_tls: opt("useTls").is_some_and(|v| v == "true"),
        options: ds.options.clone(),
    }
}

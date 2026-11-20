//! Cofres de credenciais (implementações do port `CredentialVault`).

use std::collections::HashMap;
use std::sync::RwLock;

use nova_domain::ports::CredentialVault;
use nova_domain::{DomainError, DomainResult};

/// Cofre nativo do SO: macOS Keychain, Windows Credential Manager, Linux Secret Service.
#[cfg(feature = "os-keyring")]
pub struct OsKeyringVault {
    service: String,
}

#[cfg(feature = "os-keyring")]
impl OsKeyringVault {
    pub const SERVICE_NAME: &'static str = "com.novareport.builder";

    pub fn new() -> Self {
        Self { service: Self::SERVICE_NAME.into() }
    }

    fn entry(&self, alias: &str) -> DomainResult<keyring::Entry> {
        keyring::Entry::new(&self.service, alias).map_err(|e| DomainError::Vault(e.to_string()))
    }
}

#[cfg(feature = "os-keyring")]
impl Default for OsKeyringVault {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "os-keyring")]
impl CredentialVault for OsKeyringVault {
    fn store(&self, alias: &str, secret: &str) -> DomainResult<()> {
        self.entry(alias)?.set_password(secret).map_err(|e| DomainError::Vault(e.to_string()))
    }

    fn get(&self, alias: &str) -> DomainResult<Option<String>> {
        match self.entry(alias)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(DomainError::Vault(e.to_string())),
        }
    }

    fn delete(&self, alias: &str) -> DomainResult<()> {
        match self.entry(alias)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(DomainError::Vault(e.to_string())),
        }
    }
}

/// Cofre em camadas: `get` devolve o primeiro segredo encontrado; `store`/`delete` vão para o primeiro cofre.
pub struct ChainVault {
    vaults: Vec<std::sync::Arc<dyn CredentialVault>>,
}

impl ChainVault {
    pub fn new(vaults: Vec<std::sync::Arc<dyn CredentialVault>>) -> Self {
        Self { vaults }
    }
}

impl CredentialVault for ChainVault {
    fn store(&self, alias: &str, secret: &str) -> DomainResult<()> {
        match self.vaults.first() {
            Some(v) => v.store(alias, secret),
            None => Err(DomainError::Vault("nenhum cofre configurado".into())),
        }
    }

    fn get(&self, alias: &str) -> DomainResult<Option<String>> {
        for v in &self.vaults {
            if let Some(s) = v.get(alias)? {
                return Ok(Some(s));
            }
        }
        Ok(None)
    }

    fn delete(&self, alias: &str) -> DomainResult<()> {
        self.vaults.first().map_or(Ok(()), |v| v.delete(alias))
    }
}

/// Cofre volátil (testes, CLI, modo headless).
#[derive(Default)]
pub struct MemoryVault {
    secrets: RwLock<HashMap<String, String>>,
}

impl CredentialVault for MemoryVault {
    fn store(&self, alias: &str, secret: &str) -> DomainResult<()> {
        self.secrets.write().map_err(|e| DomainError::Vault(e.to_string()))?.insert(alias.into(), secret.into());
        Ok(())
    }

    fn get(&self, alias: &str) -> DomainResult<Option<String>> {
        Ok(self.secrets.read().map_err(|e| DomainError::Vault(e.to_string()))?.get(alias).cloned())
    }

    fn delete(&self, alias: &str) -> DomainResult<()> {
        self.secrets.write().map_err(|e| DomainError::Vault(e.to_string()))?.remove(alias);
        Ok(())
    }
}

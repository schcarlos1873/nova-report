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

/// Cache em memória na frente de outro cofre: cada alias é lido do cofre interno no máximo uma vez
/// por processo (inclusive a ausência). Evita um prompt do Keychain a cada preview/consulta.
pub struct CachedVault {
    inner: std::sync::Arc<dyn CredentialVault>,
    cache: RwLock<HashMap<String, Option<String>>>,
}

impl CachedVault {
    pub fn new(inner: std::sync::Arc<dyn CredentialVault>) -> Self {
        Self { inner, cache: RwLock::default() }
    }
}

impl CredentialVault for CachedVault {
    fn store(&self, alias: &str, secret: &str) -> DomainResult<()> {
        self.inner.store(alias, secret)?;
        self.cache.write().map_err(|e| DomainError::Vault(e.to_string()))?.insert(alias.into(), Some(secret.into()));
        Ok(())
    }

    fn get(&self, alias: &str) -> DomainResult<Option<String>> {
        if let Some(hit) = self.cache.read().map_err(|e| DomainError::Vault(e.to_string()))?.get(alias) {
            return Ok(hit.clone());
        }
        // erros (ex.: acesso negado) não entram no cache: a próxima tentativa pergunta de novo
        let value = self.inner.get(alias)?;
        self.cache.write().map_err(|e| DomainError::Vault(e.to_string()))?.insert(alias.into(), value.clone());
        Ok(value)
    }

    fn delete(&self, alias: &str) -> DomainResult<()> {
        self.inner.delete(alias)?;
        self.cache.write().map_err(|e| DomainError::Vault(e.to_string()))?.insert(alias.into(), None);
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Conta leituras no cofre interno.
    #[derive(Default)]
    struct Counting {
        inner: MemoryVault,
        gets: AtomicUsize,
    }

    impl CredentialVault for Counting {
        fn store(&self, alias: &str, secret: &str) -> DomainResult<()> {
            self.inner.store(alias, secret)
        }
        fn get(&self, alias: &str) -> DomainResult<Option<String>> {
            self.gets.fetch_add(1, Ordering::SeqCst);
            self.inner.get(alias)
        }
        fn delete(&self, alias: &str) -> DomainResult<()> {
            self.inner.delete(alias)
        }
    }

    #[test]
    fn cached_vault_reads_inner_once_per_alias() {
        let inner = Arc::new(Counting::default());
        inner.store("db", "s3cr3t").unwrap();
        let v = CachedVault::new(inner.clone());
        for _ in 0..5 {
            assert_eq!(v.get("db").unwrap().as_deref(), Some("s3cr3t"));
            assert_eq!(v.get("outro").unwrap(), None);
        }
        assert_eq!(inner.gets.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn cached_vault_reflects_store_and_delete() {
        let inner = Arc::new(Counting::default());
        let v = CachedVault::new(inner.clone());
        assert_eq!(v.get("db").unwrap(), None);
        v.store("db", "nova").unwrap();
        assert_eq!(v.get("db").unwrap().as_deref(), Some("nova"));
        v.delete("db").unwrap();
        assert_eq!(v.get("db").unwrap(), None);
        assert_eq!(inner.gets.load(Ordering::SeqCst), 1);
    }
}

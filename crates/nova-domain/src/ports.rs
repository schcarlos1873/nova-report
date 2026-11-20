//! Ports (contratos) do domínio. Implementações vivem na infraestrutura
//! (`nova-connectors`, adaptadores do app desktop) e são injetadas na composição.

use std::path::Path;

use crate::DomainResult;
use crate::connection::ConnectionProfile;
use crate::report::ReportDefinition;

/// Armazenamento seguro de segredos (Keychain, Credential Manager, Secret Service...).
pub trait CredentialVault: Send + Sync {
    fn store(&self, alias: &str, secret: &str) -> DomainResult<()>;
    fn get(&self, alias: &str) -> DomainResult<Option<String>>;
    fn delete(&self, alias: &str) -> DomainResult<()>;
}

/// Persistência dos perfis de conexão (metadados sem segredos).
pub trait ConnectionProfileRepository: Send + Sync {
    fn list(&self) -> DomainResult<Vec<ConnectionProfile>>;
    fn get(&self, alias: &str) -> DomainResult<Option<ConnectionProfile>>;
    fn save(&self, profile: ConnectionProfile) -> DomainResult<()>;
    fn delete(&self, alias: &str) -> DomainResult<()>;
}

/// Persistência de definições `.nrep`.
pub trait ReportRepository: Send + Sync {
    fn load(&self, path: &Path) -> DomainResult<ReportDefinition>;
    fn save(&self, path: &Path, report: &ReportDefinition) -> DomainResult<()>;
}

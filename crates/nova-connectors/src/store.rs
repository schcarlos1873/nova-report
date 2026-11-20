//! Persistência em arquivo: perfis de conexão (`connections.json`) e definições `.nrep`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nova_domain::connection::ConnectionProfile;
use nova_domain::ports::{ConnectionProfileRepository, ReportRepository};
use nova_domain::report::ReportDefinition;
use nova_domain::{DomainError, DomainResult};

/// Perfis de conexão em JSON (sem segredos). Escrita atômica via arquivo temporário.
pub struct JsonProfileRepository {
    path: PathBuf,
    lock: Mutex<()>,
}

impl JsonProfileRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), lock: Mutex::new(()) }
    }

    fn read(&self) -> DomainResult<Vec<ConnectionProfile>> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| DomainError::Repository(e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(e) => Err(DomainError::Repository(e.to_string())),
        }
    }

    fn write(&self, profiles: &[ConnectionProfile]) -> DomainResult<()> {
        let text = serde_json::to_string_pretty(profiles).map_err(|e| DomainError::Repository(e.to_string()))?;
        write_atomic(&self.path, text.as_bytes())
    }
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> DomainResult<()> {
    let err = |e: std::io::Error| DomainError::Repository(format!("{}: {e}", path.display()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}

impl ConnectionProfileRepository for JsonProfileRepository {
    fn list(&self) -> DomainResult<Vec<ConnectionProfile>> {
        let _g = self.lock.lock().map_err(|e| DomainError::Repository(e.to_string()))?;
        let mut v = self.read()?;
        v.sort_by(|a, b| a.alias.cmp(&b.alias));
        Ok(v)
    }

    fn get(&self, alias: &str) -> DomainResult<Option<ConnectionProfile>> {
        Ok(self.list()?.into_iter().find(|p| p.alias == alias))
    }

    fn save(&self, profile: ConnectionProfile) -> DomainResult<()> {
        let _g = self.lock.lock().map_err(|e| DomainError::Repository(e.to_string()))?;
        let mut v = self.read()?;
        v.retain(|p| p.alias != profile.alias);
        v.push(profile);
        self.write(&v)
    }

    fn delete(&self, alias: &str) -> DomainResult<()> {
        let _g = self.lock.lock().map_err(|e| DomainError::Repository(e.to_string()))?;
        let mut v = self.read()?;
        v.retain(|p| p.alias != alias);
        self.write(&v)
    }
}

/// Repositório de `.nrep` no sistema de arquivos (JSON canônico).
#[derive(Default)]
pub struct FsReportRepository;

impl ReportRepository for FsReportRepository {
    fn load(&self, path: &Path) -> DomainResult<ReportDefinition> {
        let text = std::fs::read_to_string(path).map_err(|e| DomainError::Repository(format!("{}: {e}", path.display())))?;
        ReportDefinition::from_json(&text)
    }

    fn save(&self, path: &Path, report: &ReportDefinition) -> DomainResult<()> {
        write_atomic(path, report.to_canonical_json()?.as_bytes())
    }
}

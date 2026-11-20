//! # nova-domain
//!
//! Núcleo do domínio do NovaReport (camada mais interna da Clean Architecture).
//! Não depende de frameworks de I/O, banco de dados ou UI.
//!
//! * [`report`] — agregado `ReportDefinition` (formato `.nrep`), bandas, elementos e invariantes.
//! * [`connection`] — configuração de conexões e metadados de schema (navegador estilo DBeaver).
//! * [`params`] — value object [`ParamValue`] e coerção tipada de parâmetros.
//! * [`ports`] — contratos (traits) implementados pela infraestrutura.

pub mod connection;
pub mod error;
pub mod params;
pub mod ports;
pub mod report;

pub use error::{DomainError, DomainResult};
pub use params::{ParamType, ParamValue};

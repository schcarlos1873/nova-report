//! # nova-application
//!
//! Casos de uso do NovaReport Builder. Orquestra domínio + engine através de ports,
//! sem conhecer drivers, UI ou sistema operacional (injetados na composição).

pub mod connection_resolver;
pub mod error;
pub mod params;
pub mod ports;
pub mod use_cases;

pub use error::{AppError, AppResult};
pub use use_cases::connections::{ConnectionService, ParameterOption, ParameterValues, QueryPreview};
pub use use_cases::documents::DocumentService;
pub use use_cases::render::{ExportFormat, PreviewResult, RenderService, RenderStats};

//! # nova-engine
//!
//! Núcleo de processamento do NovaReport:
//!
//! ```text
//! DataProvider* ──> DataContext (Polars) ──> ReportProcessor (bandas + Rhai)
//!                                                  │
//!                                                  ▼
//!                       RenderedReport ──> TypstTranspiler ──> Typst ──> PDF / SVG
//! ```
//!
//! O engine depende apenas do domínio; drivers concretos implementam [`data::DataProvider`]
//! na camada de infraestrutura (`nova-connectors`).

pub mod data;
pub mod error;
pub mod export;
pub mod expression;
pub mod layout;
pub mod render;

pub use error::{EngineError, EngineResult};

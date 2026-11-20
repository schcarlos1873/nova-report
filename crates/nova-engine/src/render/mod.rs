//! Renderização: modelo intermediário avaliado → código Typst → documento paginado.

mod compiler;
mod model;
mod transpiler;
mod world;

pub use compiler::{CompiledReport, ReportCompiler};
pub use model::{PageConfig, RenderedBand, RenderedElement, RenderedReport, TableCell};
pub use transpiler::TypstTranspiler;
pub use world::{FontLibrary, InMemoryReportWorld};

//! Processamento de bandas: percorre o dataset principal, detecta quebras de grupo,
//! avalia expressões e produz o [`RenderedReport`].

mod assets;
mod processor;

pub use assets::AssetResolver;
pub use processor::ReportProcessor;

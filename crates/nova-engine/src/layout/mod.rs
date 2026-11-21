//! Processamento de bandas: percorre o dataset principal, detecta quebras de grupo,
//! avalia expressões e produz o [`RenderedReport`].

mod assets;
mod processor;

pub use assets::{AssetResolver, field_image_source, sniff_image_mime};
pub use processor::ReportProcessor;

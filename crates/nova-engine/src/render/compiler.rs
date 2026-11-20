use std::sync::Arc;
use std::time::Instant;

use typst::diag::SourceDiagnostic;
use typst_layout::PagedDocument;

use super::model::RenderedReport;
use super::transpiler::TypstTranspiler;
use super::world::{FontLibrary, InMemoryReportWorld};
use crate::{EngineError, EngineResult};

/// Documento paginado pronto para exportação.
pub struct CompiledReport {
    pub document: PagedDocument,
    pub typst_source: String,
    pub warnings: Vec<String>,
    pub compile_ms: u128,
}

impl CompiledReport {
    pub fn page_count(&self) -> usize {
        self.document.pages().len()
    }

    pub fn to_pdf(&self) -> EngineResult<Vec<u8>> {
        typst_pdf::pdf(&self.document, &typst_pdf::PdfOptions::default())
            .map_err(|e| EngineError::Compilation(diagnostics(&e)))
    }

    pub fn page_svg(&self, index: usize) -> Option<String> {
        self.document.pages().get(index).map(|p| typst_svg::svg(p, &typst_svg::SvgOptions::default()))
    }

    pub fn to_svg_pages(&self) -> Vec<String> {
        (0..self.page_count()).filter_map(|i| self.page_svg(i)).collect()
    }
}

/// Compila o modelo intermediário em memória. Sem estado mutável: seguro para uso concorrente.
#[derive(Clone)]
pub struct ReportCompiler {
    fonts: Arc<FontLibrary>,
}

impl ReportCompiler {
    pub fn new(fonts: Arc<FontLibrary>) -> Self {
        Self { fonts }
    }

    pub fn fonts(&self) -> &Arc<FontLibrary> {
        &self.fonts
    }

    pub fn compile(&self, report: &RenderedReport) -> EngineResult<CompiledReport> {
        let source = TypstTranspiler::generate_source(report);
        self.compile_source(source, report)
    }

    pub fn compile_source(&self, typst_source: String, report: &RenderedReport) -> EngineResult<CompiledReport> {
        let start = Instant::now();
        let world = InMemoryReportWorld::new(
            typst_source.clone(),
            report.assets.iter().map(|(k, v)| (k.clone(), v.clone())),
            Arc::clone(&self.fonts),
        );
        let warned = typst::compile::<PagedDocument>(&world);
        // libera memória de memoização antiga sem perder o cache quente
        comemo::evict(30);
        let document = warned.output.map_err(|e| EngineError::Compilation(diagnostics(&e)))?;
        let mut warnings = report.warnings.clone();
        warnings.extend(
            warned
                .warnings
                .iter()
                .filter(|w| !w.message.contains("unknown font family"))
                .map(|w| w.message.to_string()),
        );
        Ok(CompiledReport { document, typst_source, warnings, compile_ms: start.elapsed().as_millis() })
    }
}

fn diagnostics(diags: &[SourceDiagnostic]) -> String {
    diags
        .iter()
        .map(|d| {
            let hints = d.hints.iter().map(|h| format!(" (dica: {})", h.v)).collect::<String>();
            format!("{}{hints}", d.message)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

//! Caso de uso: executar queries, processar bandas e compilar (preview / exportação).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nova_domain::ParamValue;
use nova_domain::report::ReportDefinition;
use nova_engine::data::{DataContext, DataRequest};
use nova_engine::layout::ReportProcessor;
use nova_engine::render::{RenderedReport, ReportCompiler, TypstTranspiler};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::connection_resolver::ConnectionResolver;
use crate::ports::ConnectorGateway;
use crate::use_cases::connections::fetch_options;
use crate::{AppError, AppResult, params};

/// Dados reaproveitados entre previews quando só o layout muda (meta: < 50 ms).
const DATA_CACHE_TTL: Duration = Duration::from_secs(120);
const MAX_PREVIEW_PAGES: usize = 500;
/// Linhas lidas da lista de valores para achar os rótulos; valores além disso mostram o próprio valor.
const LABEL_LOOKUP_LIMIT: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Pdf,
    Xlsx,
    Csv,
    /// Código Typst intermediário (depuração / pós-processamento).
    Typst,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Xlsx => "xlsx",
            ExportFormat::Csv => "csv",
            ExportFormat::Typst => "typ",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderStats {
    pub data_ms: u128,
    pub layout_ms: u128,
    pub compile_ms: u128,
    pub total_ms: u128,
    pub data_cached: bool,
    pub rows: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub page_count: usize,
    pub pages_svg: Vec<String>,
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub render_time_ms: u128,
    pub stats: RenderStats,
    pub warnings: Vec<String>,
}

struct CacheEntry {
    key: String,
    loaded_at: Instant,
    data: DataContext,
}

#[derive(Clone)]
pub struct RenderService {
    gateway: Arc<dyn ConnectorGateway>,
    resolver: ConnectionResolver,
    compiler: ReportCompiler,
    cache: Arc<Mutex<Option<CacheEntry>>>,
}

impl RenderService {
    pub fn new(gateway: Arc<dyn ConnectorGateway>, resolver: ConnectionResolver, compiler: ReportCompiler) -> Self {
        Self { gateway, resolver, compiler, cache: Arc::new(Mutex::new(None)) }
    }

    pub fn compiler(&self) -> &ReportCompiler {
        &self.compiler
    }

    pub async fn invalidate_cache(&self) {
        *self.cache.lock().await = None;
    }

    /// Executa todas as queries (em paralelo), com cache por definição de dados + parâmetros.
    pub async fn load_data(
        &self,
        def: &ReportDefinition,
        raw_params: &HashMap<String, serde_json::Value>,
        base_dir: Option<&std::path::Path>,
        refresh: bool,
    ) -> AppResult<(DataContext, RenderStats)> {
        let parameters = params::resolve(def, raw_params)?;
        let mut sorted: Vec<_> = parameters.iter().map(|(k, v)| (k.clone(), v.to_json())).collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let key = serde_json::to_string(&(&def.data_sources, &sorted, base_dir)).unwrap_or_default();

        let mut stats = RenderStats::default();
        if !refresh {
            let cache = self.cache.lock().await;
            if let Some(c) = cache.as_ref().filter(|c| c.key == key && c.loaded_at.elapsed() < DATA_CACHE_TTL) {
                stats.data_cached = true;
                stats.rows = c.data.datasets.iter().map(|(k, v)| (k.clone(), v.height())).collect();
                return Ok((c.data.clone(), stats));
            }
        }

        let start = Instant::now();
        let mut requests = Vec::new();
        for ds in &def.data_sources {
            if ds.queries.iter().all(|q| def.is_parameter_only_query(&q.name)) {
                continue;
            }
            let config = self.resolver.for_data_source(ds, base_dir)?;
            let provider = self.gateway.provider(config)?;
            // queries que só listam valores de parâmetros não fazem parte dos dados do relatório
            for q in ds.queries.iter().filter(|q| !def.is_parameter_only_query(&q.name)) {
                requests.push(DataRequest { name: q.name.clone(), provider: provider.clone(), query: q.sql.clone() });
            }
        }
        let param_labels = self.param_labels(def, &parameters, base_dir).await?;
        let mut data = DataContext::load_all(requests, parameters).await?;
        data.param_labels = param_labels;
        stats.data_ms = start.elapsed().as_millis();
        stats.rows = data.datasets.iter().map(|(k, v)| (k.clone(), v.height())).collect();
        *self.cache.lock().await = Some(CacheEntry { key, loaded_at: Instant::now(), data: data.clone() });
        Ok((data, stats))
    }

    /// `ParamLabels.<nome>`: rótulos (`labelField`) dos valores escolhidos, buscados na query de
    /// `availableValues` com os parâmetros já resolvidos (a cascata vale aqui também).
    /// Só consulta o banco se alguma expressão usa rótulos.
    async fn param_labels(
        &self,
        def: &ReportDefinition,
        parameters: &HashMap<String, ParamValue>,
        base_dir: Option<&std::path::Path>,
    ) -> AppResult<HashMap<String, ParamValue>> {
        let text = serde_json::to_string(&def.bands).unwrap_or_default();
        if !text.contains("ParamLabels.") && !text.contains(".Label") {
            return Ok(HashMap::new());
        }
        let lookups = def.parameters.iter().filter_map(|p| {
            let has_label = p.available_values.as_ref()?.label_field.as_deref().is_some_and(|f| !f.is_empty());
            let value = parameters.get(&p.name).filter(|v| has_label && !v.is_empty())?;
            Some(async move {
                let (options, _) =
                    fetch_options(self.gateway.as_ref(), &self.resolver, def, p, parameters.clone(), base_dir, LABEL_LOOKUP_LIMIT).await?;
                Ok::<_, AppError>((p.name.clone(), params::labels_for(value, &options)))
            })
        });
        futures::future::join_all(lookups).await.into_iter().collect()
    }

    async fn render(
        &self,
        def: &ReportDefinition,
        raw_params: &HashMap<String, serde_json::Value>,
        base_dir: Option<PathBuf>,
        refresh: bool,
    ) -> AppResult<(RenderedReport, DataContext, RenderStats)> {
        def.ensure_valid()?;
        let (data, mut stats) = self.load_data(def, raw_params, base_dir.as_deref(), refresh).await?;
        let (def, data2) = (def.clone(), data.clone());
        let start = Instant::now();
        let rendered = tokio::task::spawn_blocking(move || ReportProcessor::new(&def, &data2, base_dir.as_deref()).process())
            .await
            .map_err(|e| AppError::Join(e.to_string()))??;
        stats.layout_ms = start.elapsed().as_millis();
        Ok((rendered, data, stats))
    }

    pub async fn preview(
        &self,
        def: &ReportDefinition,
        raw_params: &HashMap<String, serde_json::Value>,
        base_dir: Option<PathBuf>,
        refresh: bool,
    ) -> AppResult<PreviewResult> {
        let start = Instant::now();
        let (rendered, _, mut stats) = self.render(def, raw_params, base_dir, refresh).await?;
        let compiler = self.compiler.clone();
        let page = rendered.page.clone();
        let (pages_svg, page_count, warnings, compile_ms) = tokio::task::spawn_blocking(move || {
            let compiled = compiler.compile(&rendered)?;
            let n = compiled.page_count();
            let pages: Vec<String> = (0..n.min(MAX_PREVIEW_PAGES)).filter_map(|i| compiled.page_svg(i)).collect();
            let mut warnings = compiled.warnings.clone();
            if n > MAX_PREVIEW_PAGES {
                warnings.push(format!("pré-visualização limitada às primeiras {MAX_PREVIEW_PAGES} de {n} páginas"));
            }
            Ok::<_, AppError>((pages, n, warnings, compiled.compile_ms))
        })
        .await
        .map_err(|e| AppError::Join(e.to_string()))??;
        stats.compile_ms = compile_ms;
        stats.total_ms = start.elapsed().as_millis();
        Ok(PreviewResult {
            page_count,
            pages_svg,
            page_width_mm: page.width_mm,
            page_height_mm: page.height_mm,
            render_time_ms: stats.total_ms,
            stats,
            warnings,
        })
    }

    /// Gera o arquivo no formato pedido (sempre com dados atualizados).
    pub async fn export(
        &self,
        def: &ReportDefinition,
        raw_params: &HashMap<String, serde_json::Value>,
        base_dir: Option<PathBuf>,
        format: ExportFormat,
    ) -> AppResult<Vec<u8>> {
        match format {
            ExportFormat::Csv | ExportFormat::Xlsx => {
                def.ensure_valid()?;
                let (data, _) = self.load_data(def, raw_params, base_dir.as_deref(), true).await?;
                let main = def.main_query_name();
                let mut names: Vec<&String> = data.datasets.keys().collect();
                names.sort_by_key(|n| (Some(n.as_str()) != main.as_deref(), n.as_str().to_string()));
                tokio::task::spawn_blocking({
                    let data = data.clone();
                    let names: Vec<String> = names.into_iter().cloned().collect();
                    move || -> AppResult<Vec<u8>> {
                        if format == ExportFormat::Csv {
                            let first = names.first().ok_or_else(|| AppError::Invalid("o relatório não possui dados".into()))?;
                            Ok(nova_engine::export::to_csv(&data.datasets[first], b';')?)
                        } else {
                            let sheets: Vec<(String, &polars::prelude::DataFrame)> =
                                names.iter().map(|n| (n.clone(), &data.datasets[n])).collect();
                            Ok(nova_engine::export::to_xlsx(&sheets)?)
                        }
                    }
                })
                .await
                .map_err(|e| AppError::Join(e.to_string()))?
            }
            ExportFormat::Pdf | ExportFormat::Typst => {
                let (rendered, _, _) = self.render(def, raw_params, base_dir, true).await?;
                if format == ExportFormat::Typst {
                    return Ok(TypstTranspiler::generate_source(&rendered).into_bytes());
                }
                let compiler = self.compiler.clone();
                tokio::task::spawn_blocking(move || Ok(compiler.compile(&rendered)?.to_pdf()?))
                    .await
                    .map_err(|e| AppError::Join(e.to_string()))?
            }
        }
    }
}

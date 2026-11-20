//! Fluxo completo da camada de aplicação com o relatório de exemplo:
//! `.nrep` → resolver de conexão → driver SQLite → Polars → bandas → Typst → PDF/XLSX.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use nova_application::connection_resolver::ConnectionResolver;
use nova_application::{DocumentService, ExportFormat, RenderService};
use nova_connectors::ConnectorRegistry;
use nova_connectors::store::{FsReportRepository, JsonProfileRepository};
use nova_connectors::vault::MemoryVault;
use nova_engine::render::{FontLibrary, ReportCompiler};

static FONTS: LazyLock<Arc<FontLibrary>> = LazyLock::new(|| Arc::new(FontLibrary::load(false)));

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn services() -> (RenderService, DocumentService) {
    let profiles = std::env::temp_dir().join(format!("nova_e2e_profiles_{}.json", std::process::id()));
    let resolver = ConnectionResolver::new(Arc::new(JsonProfileRepository::new(profiles)), Arc::new(MemoryVault::default()));
    (
        RenderService::new(Arc::new(ConnectorRegistry::with_builtin_drivers()), resolver, ReportCompiler::new(FONTS.clone())),
        DocumentService::new(Arc::new(FsReportRepository)),
    )
}

#[tokio::test]
async fn example_report_previews_and_exports() {
    let (render, docs) = services();
    let path = examples().join("vendas_por_regiao.nrep");
    let (def, issues) = docs.open(&path).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let base = Some(examples());

    let params = HashMap::new(); // defaults do .nrep: ano de 2024 inteiro
    let first = render.preview(&def, &params, base.clone(), false).await.unwrap();
    assert!(first.page_count >= 5, "páginas: {}", first.page_count);
    assert_eq!(first.stats.rows["Pedidos"], 360);
    assert!(!first.stats.data_cached);
    assert!(first.warnings.is_empty(), "{:?}", first.warnings);

    // segunda renderização (mudança só de layout) reaproveita os dados
    let second = render.preview(&def, &params, base.clone(), false).await.unwrap();
    assert!(second.stats.data_cached);
    assert_eq!(second.page_count, first.page_count);

    // filtro por parâmetro reduz os dados
    let q1 = HashMap::from([("data_fim".to_string(), serde_json::json!("2024-03-31"))]);
    let filtered = render.preview(&def, &q1, base.clone(), false).await.unwrap();
    assert!(filtered.stats.rows["Pedidos"] < 360);
    assert!(filtered.page_count < first.page_count);

    let pdf = render.export(&def, &params, base.clone(), ExportFormat::Pdf).await.unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let xlsx = render.export(&def, &params, base.clone(), ExportFormat::Xlsx).await.unwrap();
    assert_eq!(&xlsx[..2], b"PK");
    let csv = String::from_utf8(render.export(&def, &params, base, ExportFormat::Csv).await.unwrap()).unwrap();
    assert_eq!(csv.lines().count(), 361);
    assert!(csv.starts_with("regiao;cliente;cidade;pedido"));
}

#[tokio::test]
async fn missing_required_parameter_is_reported() {
    let (render, docs) = services();
    let (mut def, _) = docs.open(&examples().join("vendas_por_regiao.nrep")).unwrap();
    def.parameters[0].default_value = None;
    let err = render.preview(&def, &HashMap::new(), Some(examples()), false).await.unwrap_err();
    assert!(err.user_message().contains("Data inicial"), "{}", err.user_message());
}

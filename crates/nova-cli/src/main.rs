//! `nova-report` — CLI headless do NovaReport (composição root alternativa ao desktop).
//!
//! ```text
//! nova-report render <arquivo.nrep> [-o saída] [-f pdf|xlsx|csv|typ] [-p nome=valor]... [--system-fonts]
//! nova-report validate <arquivo.nrep>
//! nova-report drivers
//! ```
//! Conexões salvas: `NOVA_CONNECTIONS` → `./connections.json` → as do desktop (diretório de config do SO).
//! Segredos: `NOVA_SECRET_<ALIAS>` → cofre do SO (o mesmo usado pelo desktop).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use nova_application::connection_resolver::ConnectionResolver;
use nova_application::ports::ConnectorGateway;
use nova_application::{DocumentService, ExportFormat, RenderService};
use nova_connectors::ConnectorRegistry;
use nova_connectors::store::{FsReportRepository, JsonProfileRepository};
use nova_connectors::vault::{CachedVault, ChainVault, MemoryVault, OsKeyringVault};
use nova_domain::ports::CredentialVault;
use nova_domain::report::Severity;
use nova_engine::render::{FontLibrary, ReportCompiler};

const USAGE: &str = "uso:
  nova-report render <arquivo.nrep> [-o saída] [-f pdf|xlsx|csv|typ] [-p nome=valor]... [--system-fonts]
  nova-report validate <arquivo.nrep>
  nova-report svg <arquivo.nrep> <diretório> [-p nome=valor]...
  nova-report drivers

conexões: NOVA_CONNECTIONS, ./connections.json ou as salvas no desktop
senhas:   NOVA_SECRET_<ALIAS> ou o cofre do sistema (Keychain/Credential Manager/Secret Service)";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("render") => render(&args[1..]).await,
        Some("validate") => validate(&args[1..]),
        Some("svg") => svg_pages(&args[1..]).await,
        Some("drivers") => {
            for d in ConnectorRegistry::with_builtin_drivers().drivers() {
                println!("{:<10} {:<10} {}", d.id, d.tier, d.name);
            }
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn validate(args: &[String]) -> Result<(), String> {
    let path = args.first().ok_or(USAGE)?;
    let docs = DocumentService::new(Arc::new(FsReportRepository));
    let (_, issues) = docs.open(Path::new(path)).map_err(|e| e.user_message())?;
    for i in &issues {
        println!("{i}");
    }
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    println!("{} erro(s), {} aviso(s)", errors, issues.len() - errors);
    if errors > 0 { Err("relatório inválido".into()) } else { Ok(()) }
}

/// Composição dos serviços para execução headless.
fn compose(system_fonts: bool) -> Result<(RenderService, DocumentService), String> {
    let env_vault = Arc::new(MemoryVault::default());
    for (k, v) in std::env::vars() {
        if let Some(alias) = k.strip_prefix("NOVA_SECRET_") {
            env_vault.store(alias, &v).map_err(|e| e.to_string())?;
        }
    }
    let vault = Arc::new(ChainVault::new(vec![env_vault, Arc::new(CachedVault::new(Arc::new(OsKeyringVault::new())))]));
    let resolver = ConnectionResolver::new(Arc::new(JsonProfileRepository::new(profiles_path())), vault);
    let fonts = Arc::new(FontLibrary::load(system_fonts));
    let render = RenderService::new(Arc::new(ConnectorRegistry::with_builtin_drivers()), resolver, ReportCompiler::new(fonts));
    Ok((render, DocumentService::new(Arc::new(FsReportRepository))))
}

/// `NOVA_CONNECTIONS` → `./connections.json` → arquivo do desktop (mesmo `app_config_dir` do Tauri).
fn profiles_path() -> PathBuf {
    if let Ok(p) = std::env::var("NOVA_CONNECTIONS") {
        return p.into();
    }
    let local = PathBuf::from("connections.json");
    if local.exists() {
        return local;
    }
    dirs::config_dir().map_or(local, |d| d.join(OsKeyringVault::SERVICE_NAME).join("connections.json"))
}

/// Gera as páginas do preview em SVG (mesmo caminho usado pelo visualizador do desktop).
async fn svg_pages(args: &[String]) -> Result<(), String> {
    let (input, dir) = match args {
        [i, d, ..] => (PathBuf::from(i), PathBuf::from(d)),
        _ => return Err(USAGE.into()),
    };
    let mut params = HashMap::new();
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        if a == "-p" {
            let (k, v) = it.next().and_then(|kv| kv.split_once('=')).ok_or("-p exige nome=valor")?;
            params.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
    }
    let (render, docs) = compose(true)?;
    let (def, _) = docs.open(&input).map_err(|e| e.user_message())?;
    let res = render.preview(&def, &params, input.parent().map(Path::to_path_buf), false).await.map_err(|e| e.user_message())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (i, svg) in res.pages_svg.iter().enumerate() {
        std::fs::write(dir.join(format!("page_{:03}.svg", i + 1)), svg).map_err(|e| e.to_string())?;
    }
    println!("{} página(s) · {:?}", res.page_count, res.stats);
    for w in res.warnings {
        println!("aviso: {w}");
    }
    Ok(())
}

async fn render(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut output = None;
    let mut format = None;
    let mut params = HashMap::new();
    let mut system_fonts = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--output" => output = it.next().map(PathBuf::from),
            "-f" | "--format" => {
                format = Some(match it.next().map(String::as_str) {
                    Some("pdf") => ExportFormat::Pdf,
                    Some("xlsx") => ExportFormat::Xlsx,
                    Some("csv") => ExportFormat::Csv,
                    Some("typ") => ExportFormat::Typst,
                    other => return Err(format!("formato inválido: {other:?}")),
                })
            }
            "-p" | "--param" => {
                let kv = it.next().ok_or("-p exige nome=valor")?;
                let (k, v) = kv.split_once('=').ok_or("-p exige nome=valor")?;
                params.insert(k.to_string(), serde_json::Value::String(v.to_string()));
            }
            "--system-fonts" => system_fonts = true,
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => return Err(format!("argumento desconhecido: {other}\n{USAGE}")),
        }
    }
    let input = input.ok_or(USAGE)?;
    let format = format
        .or_else(|| output.as_ref().and_then(|o| o.extension()).and_then(|e| match e.to_str()? {
            "xlsx" => Some(ExportFormat::Xlsx),
            "csv" => Some(ExportFormat::Csv),
            "typ" => Some(ExportFormat::Typst),
            _ => Some(ExportFormat::Pdf),
        }))
        .unwrap_or(ExportFormat::Pdf);
    let output = output.unwrap_or_else(|| input.with_extension(format.extension()));

    let (render, docs) = compose(system_fonts)?;

    let started = std::time::Instant::now();
    let (def, _) = docs.open(&input).map_err(|e| e.user_message())?;
    let base_dir = input.parent().map(Path::to_path_buf);
    let bytes = render.export(&def, &params, base_dir, format).await.map_err(|e| e.user_message())?;
    std::fs::write(&output, &bytes).map_err(|e| format!("{}: {e}", output.display()))?;
    println!("{} gerado ({} bytes) em {} ms", output.display(), bytes.len(), started.elapsed().as_millis());
    Ok(())
}

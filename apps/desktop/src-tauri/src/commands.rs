//! Comandos IPC (camada de apresentação). Apenas traduzem entrada/saída; regras
//! de negócio vivem nos casos de uso.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use nova_application::ports::DriverDescriptor;
use nova_application::{AppError, ExportFormat, ParameterValues, PreviewResult, QueryPreview};
use nova_domain::connection::{ConnectionProfile, DatabaseTree};
use nova_domain::report::{ReportDefinition, ValidationIssue};
use serde::Serialize;
use tauri::State;

use crate::AppState;

type Params = HashMap<String, serde_json::Value>;
type CmdResult<T> = Result<T, AppError>;

fn base_dir(report_path: &Option<String>) -> Option<PathBuf> {
    report_path.as_ref().and_then(|p| Path::new(p).parent().map(Path::to_path_buf))
}

/// `Report.FileName`/`Report.FilePath` nas expressões.
fn with_source(mut report: ReportDefinition, report_path: &Option<String>) -> ReportDefinition {
    report.source_path = report_path.as_ref().map(PathBuf::from);
    report
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: &'static str,
    fonts: Vec<String>,
    drivers: Vec<DriverDescriptor>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo { version: env!("CARGO_PKG_VERSION"), fonts: state.fonts.families(), drivers: state.connections.drivers() }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedReport {
    report: ReportDefinition,
    issues: Vec<ValidationIssue>,
}

#[tauri::command]
pub fn new_report(state: State<'_, AppState>, title: Option<String>) -> ReportDefinition {
    state.documents.new_report(title.as_deref().unwrap_or("Novo relatório"))
}

#[tauri::command]
pub fn open_report(state: State<'_, AppState>, path: String) -> CmdResult<OpenedReport> {
    let (report, issues) = state.documents.open(Path::new(&path))?;
    Ok(OpenedReport { report, issues })
}

#[tauri::command]
pub async fn save_report(state: State<'_, AppState>, path: String, report: ReportDefinition) -> CmdResult<Vec<ValidationIssue>> {
    state.documents.save(Path::new(&path), &report)
}

#[tauri::command]
pub fn validate_report(state: State<'_, AppState>, report: ReportDefinition) -> Vec<ValidationIssue> {
    state.documents.validate(&report)
}

#[tauri::command]
pub fn check_expression(state: State<'_, AppState>, expression: String) -> Result<(), String> {
    state.documents.check_expression(&expression)
}

/// Executa e compila em thread de bloqueio (a UI nunca trava).
#[tauri::command]
pub async fn render_report_preview(
    state: State<'_, AppState>,
    report: ReportDefinition,
    params: Params,
    report_path: Option<String>,
    refresh: Option<bool>,
) -> CmdResult<PreviewResult> {
    let report = with_source(report, &report_path);
    state.render.preview(&report, &params, base_dir(&report_path), refresh.unwrap_or(false)).await
}

#[tauri::command]
pub async fn export_report(
    state: State<'_, AppState>,
    report: ReportDefinition,
    params: Params,
    report_path: Option<String>,
    format: ExportFormat,
    output_path: String,
) -> CmdResult<usize> {
    let report = with_source(report, &report_path);
    let bytes = state.render.export(&report, &params, base_dir(&report_path), format).await?;
    tokio::fs::write(&output_path, &bytes).await.map_err(|e| AppError::Invalid(format!("{output_path}: {e}")))?;
    Ok(bytes.len())
}

#[tauri::command]
pub fn list_drivers(state: State<'_, AppState>) -> Vec<DriverDescriptor> {
    state.connections.drivers()
}

#[tauri::command]
pub fn list_connections(state: State<'_, AppState>) -> CmdResult<Vec<ConnectionProfile>> {
    state.connections.list()
}

#[tauri::command]
pub async fn save_connection(state: State<'_, AppState>, profile: ConnectionProfile, password: Option<String>) -> CmdResult<()> {
    state.connections.save(profile, password)?;
    state.render.invalidate_cache().await;
    Ok(())
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, alias: String) -> CmdResult<()> {
    state.connections.delete(&alias)?;
    state.render.invalidate_cache().await;
    Ok(())
}

#[tauri::command]
pub async fn test_database_connection(state: State<'_, AppState>, profile: ConnectionProfile, password: Option<String>) -> CmdResult<u64> {
    state.connections.test(profile, password).await
}

#[tauri::command]
pub async fn fetch_schema_tree(state: State<'_, AppState>, alias: String) -> CmdResult<DatabaseTree> {
    state.connections.schema(&alias).await
}

#[tauri::command]
pub fn select_all_sql(state: State<'_, AppState>, alias: String, schema: Option<String>, table: String) -> CmdResult<String> {
    state.connections.select_all_sql(&alias, schema.as_deref(), &table)
}

#[tauri::command]
pub async fn preview_sql(state: State<'_, AppState>, alias: String, sql: String, params: Option<Params>, limit: Option<usize>) -> CmdResult<QueryPreview> {
    state.connections.preview_sql(&alias, &sql, &params.unwrap_or_default(), limit.unwrap_or(100)).await
}

#[tauri::command]
pub async fn preview_report_query(
    state: State<'_, AppState>,
    report: ReportDefinition,
    query_name: String,
    params: Option<Params>,
    report_path: Option<String>,
    limit: Option<usize>,
) -> CmdResult<QueryPreview> {
    state
        .connections
        .preview_report_query(&report, &query_name, &params.unwrap_or_default(), base_dir(&report_path).as_deref(), limit.unwrap_or(100))
        .await
}

/// Lista de valores de um parâmetro (filtros em cascata), já respeitando `dependsOn`.
#[tauri::command]
pub async fn parameter_values(
    state: State<'_, AppState>,
    report: ReportDefinition,
    name: String,
    params: Option<Params>,
    report_path: Option<String>,
    limit: Option<usize>,
) -> CmdResult<ParameterValues> {
    state
        .connections
        .parameter_values(&report, &name, &params.unwrap_or_default(), base_dir(&report_path).as_deref(), limit.unwrap_or(1000))
        .await
}

/// Lê uma imagem local como `data:` URI (imagens embutidas no `.nrep`).
#[tauri::command]
pub async fn read_file_data_uri(path: String) -> CmdResult<String> {
    use base64::Engine as _;
    const MAX_BYTES: usize = 5 * 1024 * 1024;
    let bytes = tokio::fs::read(&path).await.map_err(|e| AppError::Invalid(format!("{path}: {e}")))?;
    if bytes.len() > MAX_BYTES {
        return Err(AppError::Invalid(format!("{path}: imagem maior que 5 MB; use o tipo Local")));
    }
    let mime = nova_engine::layout::sniff_image_mime(&bytes).ok_or_else(|| AppError::Invalid(format!("{path}: formato de imagem não reconhecido")))?;
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

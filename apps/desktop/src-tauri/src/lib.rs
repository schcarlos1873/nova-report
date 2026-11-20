//! Composition root do NovaReport Builder: instancia infraestrutura, injeta nos casos de
//! uso e expõe tudo à UI via comandos IPC do Tauri.

mod commands;
mod state;

use std::sync::Arc;

use nova_application::connection_resolver::ConnectionResolver;
use nova_application::{ConnectionService, DocumentService, RenderService};
use nova_connectors::ConnectorRegistry;
use nova_connectors::store::{FsReportRepository, JsonProfileRepository};
use nova_connectors::vault::{CachedVault, OsKeyringVault};
use nova_engine::render::{FontLibrary, ReportCompiler};
use tauri::Manager;

pub use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&config_dir)?;

            let registry = Arc::new(ConnectorRegistry::with_builtin_drivers());
            let resolver = ConnectionResolver::new(
                Arc::new(JsonProfileRepository::new(config_dir.join("connections.json"))),
                // Keychain lido uma vez por alias na sessão (senão o macOS pergunta a cada preview).
                Arc::new(CachedVault::new(Arc::new(OsKeyringVault::new()))),
            );
            // Fontes do sistema carregadas uma vez (fidelidade WYSIWYG entre designer e PDF).
            let fonts = Arc::new(FontLibrary::load(true));
            let state = AppState {
                documents: DocumentService::new(Arc::new(FsReportRepository)),
                render: RenderService::new(registry.clone(), resolver.clone(), ReportCompiler::new(fonts.clone())),
                connections: ConnectionService::new(registry, resolver),
                fonts,
            };
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::new_report,
            commands::open_report,
            commands::save_report,
            commands::validate_report,
            commands::check_expression,
            commands::render_report_preview,
            commands::export_report,
            commands::list_drivers,
            commands::list_connections,
            commands::save_connection,
            commands::delete_connection,
            commands::test_database_connection,
            commands::fetch_schema_tree,
            commands::select_all_sql,
            commands::preview_sql,
            commands::preview_report_query,
            commands::parameter_values,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o NovaReport Builder");
}

use std::sync::Arc;

use nova_application::{ConnectionService, DocumentService, RenderService};
use nova_engine::render::FontLibrary;

/// Serviços de aplicação compartilhados entre os comandos IPC.
pub struct AppState {
    pub documents: DocumentService,
    pub render: RenderService,
    pub connections: ConnectionService,
    pub fonts: Arc<FontLibrary>,
}

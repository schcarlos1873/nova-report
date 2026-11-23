use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

use nova_application::{ConnectionService, DocumentService, RenderService};
use nova_engine::render::FontLibrary;

/// Serviços de aplicação compartilhados entre os comandos IPC.
pub struct AppState {
    pub documents: DocumentService,
    pub render: RenderService,
    pub connections: ConnectionService,
    pub fonts: Arc<FontLibrary>,
    /// Testes de conexão em andamento (id da requisição → sinal de cancelamento).
    pub pending_tests: Mutex<HashMap<String, Arc<Notify>>>,
}

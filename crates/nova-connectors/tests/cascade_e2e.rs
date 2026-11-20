//! Filtros em cascata com o banco de exemplo: Região → Cliente (vários valores) → relatório.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use nova_application::connection_resolver::ConnectionResolver;
use nova_application::{ConnectionService, DocumentService, RenderService};
use nova_connectors::ConnectorRegistry;
use nova_connectors::store::{FsReportRepository, JsonProfileRepository};
use nova_connectors::vault::MemoryVault;
use nova_engine::render::{FontLibrary, ReportCompiler};
use serde_json::json;

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn services() -> (ConnectionService, RenderService, DocumentService) {
    let profiles = std::env::temp_dir().join(format!("nova_cascade_profiles_{}.json", std::process::id()));
    let resolver = ConnectionResolver::new(Arc::new(JsonProfileRepository::new(profiles)), Arc::new(MemoryVault::default()));
    let registry = Arc::new(ConnectorRegistry::with_builtin_drivers());
    (
        ConnectionService::new(registry.clone(), resolver.clone()),
        RenderService::new(registry, resolver, ReportCompiler::new(Arc::new(FontLibrary::load(false)))),
        DocumentService::new(Arc::new(FsReportRepository)),
    )
}

#[tokio::test]
async fn cascading_lists_follow_parent_selection() {
    let (conn, render, docs) = services();
    let (def, issues) = docs.open(&examples().join("vendas_cascata.nrep")).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let base = Some(examples());

    let regioes = conn.parameter_values(&def, "regioes", &HashMap::new(), base.as_deref(), 1000).await.unwrap();
    assert!(regioes.values.len() >= 2, "{regioes:?}");
    assert!(regioes.values.iter().all(|o| o.value.is_i64()), "Id da região deve vir como inteiro");

    // filho aguarda o pai (seleção vazia explícita; sem ela valeria o padrão do .nrep)
    let nenhuma = HashMap::from([("regioes".to_string(), json!([]))]);
    let esperando = conn.parameter_values(&def, "clientes", &nenhuma, base.as_deref(), 1000).await.unwrap();
    assert!(esperando.values.is_empty() && !esperando.waiting_for.is_empty());

    // ordem por nome: [1] = Nordeste (3 clientes), [2] = Sudeste
    let uma = HashMap::from([("regioes".to_string(), json!([regioes.values[1].value]))]);
    let duas = HashMap::from([("regioes".to_string(), json!([regioes.values[1].value, regioes.values[2].value]))]);
    let c1 = conn.parameter_values(&def, "clientes", &uma, base.as_deref(), 1000).await.unwrap();
    let c2 = conn.parameter_values(&def, "clientes", &duas, base.as_deref(), 1000).await.unwrap();
    assert!(c1.values.len() > 1 && c2.values.len() > c1.values.len(), "{} vs {}", c1.values.len(), c2.values.len());

    // limite e truncamento
    let t = conn.parameter_values(&def, "clientes", &duas, base.as_deref(), 1).await.unwrap();
    assert!(t.truncated && t.values.len() == 1);

    // relatório filtrado pelas listas; queries de lista não são executadas na geração
    let mut params = uma.clone();
    params.insert("clientes".into(), json!([c1.values[0].value]));
    let r = render.preview(&def, &params, base.clone(), true).await.unwrap();
    assert!(!r.stats.rows.contains_key("LkRegioes") && !r.stats.rows.contains_key("LkClientes"), "{:?}", r.stats.rows);
    let filtrado = r.stats.rows["Pedidos"];
    params.insert("clientes".into(), json!(c1.values.iter().map(|o| o.value.clone()).collect::<Vec<_>>()));
    let todos = render.preview(&def, &params, base, true).await.unwrap().stats.rows["Pedidos"];
    assert!(filtrado > 0 && todos > filtrado, "{filtrado} vs {todos}");
}

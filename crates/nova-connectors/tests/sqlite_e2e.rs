use std::collections::HashMap;

use nova_connectors::ConnectorRegistry;
use nova_domain::ParamValue;
use nova_domain::connection::{ConnectionConfig, ConnectionProfile};
use nova_domain::report::ProviderKind;

fn temp_db() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nova_conn_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.db");
    let _ = std::fs::remove_file(&path);
    let c = rusqlite_conn(&path);
    c.execute_batch(
        "CREATE TABLE clientes (id INTEGER PRIMARY KEY, nome TEXT NOT NULL);
         CREATE TABLE pedidos (id INTEGER PRIMARY KEY, cliente_id INTEGER REFERENCES clientes(id), valor REAL, data DATE);
         CREATE VIEW v_total AS SELECT cliente_id, SUM(valor) total FROM pedidos GROUP BY cliente_id;
         INSERT INTO clientes VALUES (1, 'Ana'), (2, 'Bruno');
         INSERT INTO pedidos VALUES (1, 1, 10.5, '2024-01-10'), (2, 2, 20, '2024-02-01'), (3, 1, 5, '2024-03-15');",
    )
    .unwrap();
    path
}

fn rusqlite_conn(p: &std::path::Path) -> rusqlite::Connection {
    rusqlite::Connection::open(p).unwrap()
}

fn config(path: &std::path::Path) -> ConnectionConfig {
    ConnectionConfig::new(
        ConnectionProfile {
            alias: "local".into(),
            provider: ProviderKind::Sqlite,
            host: None,
            port: None,
            database: None,
            username: None,
            path: Some(path.to_string_lossy().into()),
            connection_string: None,
            use_tls: false,
            options: Default::default(),
        },
        None,
    )
}

#[tokio::test]
async fn sqlite_query_with_named_params_and_introspection() {
    let path = temp_db();
    let registry = ConnectorRegistry::with_builtin_drivers();
    let driver = registry.get(ProviderKind::Sqlite).unwrap();
    let cfg = config(&path);

    driver.test_connection(&cfg).await.unwrap();

    let provider = driver.provider(cfg.clone()).unwrap();
    let params = HashMap::from([("inicio".to_string(), ParamValue::Text("2024-02-01".into()))]);
    let df = provider
        .execute_query("SELECT p.id, c.nome, p.valor, p.data FROM pedidos p JOIN clientes c ON c.id = p.cliente_id WHERE p.data >= :inicio ORDER BY p.id", &params)
        .await
        .unwrap();
    assert_eq!(df.height(), 2);
    assert_eq!(df.column("data").unwrap().dtype(), &polars::prelude::DataType::Date);
    assert_eq!(df.column("valor").unwrap().dtype(), &polars::prelude::DataType::Float64);

    let tree = driver.introspect(&cfg).await.unwrap();
    let main = &tree.schemas[0];
    assert_eq!(main.tables.len(), 2);
    assert_eq!(main.views.len(), 1);
    let pedidos = main.tables.iter().find(|t| t.name == "pedidos").unwrap();
    assert!(pedidos.columns.iter().any(|c| c.name == "id" && c.is_primary_key));
    assert!(pedidos.columns.iter().any(|c| c.name == "cliente_id" && c.is_foreign_key));
}

#[test]
fn registry_lists_builtin_drivers() {
    let ids: Vec<_> = ConnectorRegistry::with_builtin_drivers().list().into_iter().map(|d| d.id).collect();
    for expected in ["sqlite", "postgres", "mysql", "mssql", "csv", "json", "rest"] {
        assert!(ids.contains(&expected), "{expected} ausente em {ids:?}");
    }
}

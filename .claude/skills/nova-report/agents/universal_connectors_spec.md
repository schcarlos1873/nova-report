# Spec de Engenharia: Arquitetura de Conectores Universais (`report-connectors`)

**Versão:** 1.0  
**Escopo:** Módulo compartilhado para **NovaReport Builder (Tauri Desktop)** e **NovaReport Server (Axum/Tonic Portal)**  
**Linguagem & Stack:** Rust | Polars / Apache Arrow | ODBC / ADBC | gRPC (Tonic) | Keyring & AES-256-GCM  

---

## 1. Visão Geral da Arquitetura

O módulo `report-connectors` é uma crate Rust isolada projetada para atuar como a camada unificada de acesso a dados do ecossistema NovaReport. Inspirada na flexibilidade de ecossistemas como o DBeaver, mas sem a sobrecarga de memória da JVM/JDBC, esta arquitetura abstrai qualquer fonte de dados heterogênea em estruturas tabulares de alta performance baseadas em **Apache Arrow** e **Polars DataFrames**.

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                       NovaReport Application Layer                          │
│                (Tauri Desktop Builder / Axum Server Portal)                 │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            `report-connectors`                              │
│                    Unified Data Access Layer & Engine                       │
├──────────────────────────────┬──────────────────────────────┬───────────────┤
│    Tier 1: Native Drivers    │    Tier 2: ODBC / ADBC      │ Tier 3: gRPC  │
│  (Postgres, MSSQL, MySQL)    │  (Oracle, DB2, SAP, Files)   │ (Custom/Side) │
└──────────────┬───────────────┴──────────────┬───────────────┴───────┬───────┘
               │                              │                       │
               ▼                              ▼                       ▼
┌──────────────────────────────┐┌──────────────────────────────┐┌──────────────┐
│ Direct Socket / TCP / Async  ││ System Driver Manager (ODBC) ││ External App │
└──────────────────────────────┘└──────────────────────────────┘└──────────────┘
```

---

## 2. Camadas de Drivers (Multi-Tier Driver Architecture)

### Tier 1: Drivers Nativos (Built-in Zero-Config)
Compilados diretamente na crate em Rust puro com conectividade assíncrona baseada em `tokio`. Oferecem a menor latência e zero dependência de instalação no sistema operacional.

* **PostgreSQL / TimescaleDB / CockroachDB:** via `sqlx` (suporte a SSL/TLS, conexões preparadas e streaming).
* **Microsoft SQL Server / Azure SQL:** via `tiberius` (suporte a Windows Auth/NTLM, TLS, TDS protocol).
* **MySQL / MariaDB:** via `sqlx` / `mysql_async`.
* **SQLite / DuckDB:** via `rusqlite` / `duckdb` (ideal para arquivos locais ou cache temporário em memória).
* **ClickHouse:** via `clickhouse-rs` (para relatórios analíticos OLAP de alta volumetria).

### Tier 2: Ponte Universal ODBC e ADBC
Para bancos legados ou proprietários onde não existe driver Rust puro disponível.

* **ODBC Engine (`odbc-api`):** Conecta a qualquer DSN ou Driver String instalado no Windows (`odbc32.dll`), Linux (`unixODBC`) ou macOS (`iODBC`). Permite integrar **Oracle, IBM DB2, Sybase, Firebird, Progress OpenEdge, SAP HANA e arquivos Access/Excel**.
* **ADBC Engine (Arrow Database Connectivity):** Padrão moderno da Apache Software Foundation para ingestão colunar direta de sistemas como **Snowflake, Databricks e BigQuery** sem passar por conversão de linhas.

### Tier 3: Plugins Dinâmicos Sidecar via gRPC
Permite estender o sistema com conectores customizados compilados separadamente (em Rust, Go, Python ou C#) que rodam como processos *sidecar* isolados.

#### Contrato Proto (gRPC Protocol Definition)
```protobuf
syntax = "proto3";
package novareport.connectors.v1;

service ConnectorPlugin {
  rpc TestConnection (ConnectionRequest) returns (TestResponse);
  rpc IntrospectSchema (IntrospectRequest) returns (SchemaResponse);
  rpc ExecuteQuery (QueryRequest) returns (stream ArrowRecordBatchChunk);
}

message ConnectionRequest {
  string driver_id = 1;
  map<string, string> config = 2;
}

message TestResponse {
  bool success = 1;
  uint64 latency_ms = 2;
  string error_message = 3;
}

message IntrospectRequest {
  map<string, string> config = 1;
  string target_schema = 2;
}

message ColumnInfo {
  string name = 1;
  string native_type = 2;
  string arrow_type = 3;
  bool nullable = 4;
  bool is_pk = 5;
}

message TableInfo {
  string schema = 1;
  string name = 2;
  string kind = 3; // TABLE, VIEW, STORED_PROCEDURE
  repeated ColumnInfo columns = 4;
}

message SchemaResponse {
  repeated TableInfo tables = 1;
}

message QueryRequest {
  map<string, string> config = 1;
  string query = 2;
  map<string, string> parameters = 3;
}

message ArrowRecordBatchChunk {
  bytes arrow_ipc_bytes = 1; // Zero-copy Apache Arrow Stream Payload
}
```

---

## 3. Motor de Introspecção & Navegação de Metadados (DBeaver Style)

Para fornecer a experiência de navegação visual por bancos de dados no estilo DBeaver, a crate expõe metadados estruturados para renderização de árvores no frontend (`Database Navigator`).

### Estruturas de Metadados em Rust

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseTree {
    pub database_name: String,
    pub schemas: Vec<SchemaMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaMetadata {
    pub name: String,
    pub tables: Vec<TableMetadata>,
    pub views: Vec<TableMetadata>,
    pub procedures: Vec<ProcedureMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    pub schema: String,
    pub name: String,
    pub comment: Option<String>,
    pub columns: Vec<ColumnMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMetadata {
    pub name: String,
    pub native_type: String,
    pub arrow_type: String, // Normalizado para Apache Arrow
    pub nullable: bool,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureMetadata {
    pub schema: String,
    pub name: String,
    pub parameters: Vec<ProcedureParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureParam {
    pub name: String,
    pub data_type: String,
    pub direction: String, // "IN", "OUT", "INOUT"
}
```

---

## 4. Abstração de Dialeto SQL & Query Builder

Para permitir consultas adaptáveis entre diferentes SGBDs, a crate inclui um gerador e parser de dialetos SQL via `sqlparser-rs` e a abstração `SqlDialect`.

```rust
pub trait SqlDialect: Send + Sync {
    fn quote_identifier(&self, id: &str) -> String;
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String;
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String;
}

pub struct PostgresDialect;
impl SqlDialect for PostgresDialect {
    fn quote_identifier(&self, id: &str) -> String {
        format!("\"{}\"", id)
    }
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        format!("{} LIMIT {} OFFSET {}", query, limit, offset)
    }
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String {
        format!("'{}'::date", date.format("%Y-%m-%d"))
    }
}

pub struct MsSqlDialect;
impl SqlDialect for MsSqlDialect {
    fn quote_identifier(&self, id: &str) -> String {
        format!("[{}]", id)
    }
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        format!("{} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", query, offset, limit)
    }
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String {
        format!("'{}'", date.format("%Y-%m-%d"))
    }
}
```

---

## 5. Cofre de Credenciais Seguro (Desktop vs. Portal)

A segurança de dados de acesso é estritamente separada entre o ambiente de desenvolvimento desktop e a execução enterprise no portal server. O arquivo `.nrep` **nunca armazena senhas em texto claro**.

```text
                   .nrep Report File (Metadata Only)
                  ┌─────────────────────────────────┐
                  │ DataSource Alias: "ERP_PROD"    │
                  │ (No passwords or tokens stored) │
                  └────────────────┬────────────────┘
                                   │
              ┌────────────────────┴────────────────────┐
              ▼                                         ▼
   Desktop (Tauri Engine)                    Portal (Server Engine)
┌───────────────────────────┐             ┌───────────────────────────┐
│ OS Keyring API Integration│             │ Central Vault & AES-256   │
│ - Windows Credential Mgr  │             │ - HashiCorp Vault / AWS   │
│ - macOS Keychain          │             │ - Postgres DB Encrypted   │
│ - Linux Secret Service    │             │ - Env Injection           │
└───────────────────────────┘             └───────────────────────────┘
```

### 5.1 Implementação Desktop (Tauri Keyring)
Usa o mecanismo de segurança nativo do SO via crate `keyring`:

```rust
use keyring::Entry;

pub struct DesktopCredentialVault;

impl DesktopCredentialVault {
    const SERVICE_NAME: &'static str = "com.novareport.builder";

    pub fn store_password(ds_alias: &str, password: &str) -> Result<(), String> {
        let entry = Entry::new(Self::SERVICE_NAME, ds_alias).map_err(|e| e.to_string())?;
        entry.set_password(password).map_err(|e| e.to_string())
    }

    pub fn get_password(ds_alias: &str) -> Result<String, String> {
        let entry = Entry::new(Self::SERVICE_NAME, ds_alias).map_err(|e| e.to_string())?;
        entry.get_password().map_err(|e| e.to_string())
    }
}
```

### 5.2 Implementação Portal (Server Vault / AES-256-GCM)
No Portal Enterprise (Fase 2), as credenciais são criptografadas com chave mestre rotacionável:

```rust
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

pub struct PortalCredentialVault {
    cipher: Aes256Gcm,
}

impl PortalCredentialVault {
    pub fn new(master_key_32_bytes: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(master_key_32_bytes.into());
        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, String> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| format!("Erro de criptografia: {:?}", e))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, payload: &[u8]) -> Result<String, String> {
        if payload.len() < 12 {
            return Err("Payload inválido".into());
        }
        let (nonce_bytes, ciphertext) = payload.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext_bytes = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Erro de descriptografia: {:?}", e))?;

        String::from_utf8(plaintext_bytes).map_err(|e| e.to_string())
    }
}
```

---

## 6. Integração Tauri (Desktop) e Axum (Portal Server)

### 6.1 Exposição IPC para a UI do Builder (Tauri Command)

```rust
#[tauri::command]
pub async fn test_database_connection(
    config: ConnectionConfig,
) -> Result<u64, String> {
    let driver = crate::registry::get_driver(&config.driver_id)?;
    driver.test_connection(&config).await
}

#[tauri::command]
pub async fn fetch_schema_tree(
    config: ConnectionConfig,
) -> Result<DatabaseTree, String> {
    let driver = crate::registry::get_driver(&config.driver_id)?;
    let tables = driver.introspect_schema(&config).await?;
    
    // Converte lista plana em árvore de navegação
    Ok(crate::metadata::build_tree(tables))
}
```

### 6.2 Endpoint REST do Portal Server (Axum Handler)

```rust
use axum::{extract::State, Json};

pub async fn handle_test_connection(
    State(vault): State<Arc<PortalCredentialVault>>,
    Json(payload): Json<ConnectionConfig>,
) -> Result<Json<TestResultResponse>, AppError> {
    let driver = registry::get_driver(&payload.driver_id)?;
    let latency = driver.test_connection(&payload).await?;
    Ok(Json(TestResultResponse { success: true, latency_ms: latency }))
}
```

---

## 7. Resumo da Matriz de Drivers & Recursos

| Driver / Protocolo | Status de Integração | Introspecção de Schema | Ingestão Direct Stream | Suporte Cross-Tab / Polars |
| :--- | :--- | :--- | :--- | :--- |
| **PostgreSQL** | Nativo (`sqlx`) | Sim (`information_schema`) | Sim (`Arrow IPC`) | Total |
| **MS SQL Server** | Nativo (`tiberius`) | Sim (`sys.tables / sys.columns`) | Sim (`Arrow IPC`) | Total |
| **MySQL / MariaDB** | Nativo (`sqlx`) | Sim (`information_schema`) | Sim (`Arrow IPC`) | Total |
| **SQLite / DuckDB** | Nativo (`rusqlite`) | Sim (`PRAGMA table_info`) | Sim (`Zero-copy`) | Total |
| **ODBC (Generic)** | Ponte (`odbc-api`) | Sim (`SQLTables / SQLColumns`) | Sim (`Batch Fetch`) | Total |
| **gRPC Sidecar** | Protocolo (`Tonic`) | Sim (`IntrospectSchema RPC`) | Sim (`Arrow Stream RPC`) | Total |

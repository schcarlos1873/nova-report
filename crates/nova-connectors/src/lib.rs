//! # nova-connectors (`report-connectors`)
//!
//! Camada unificada de acesso a dados (infraestrutura). Toda fonte heterogênea é
//! convertida em `polars::DataFrame` via [`nova_engine::data::DataProvider`].
//!
//! | Tier | Drivers |
//! | :--- | :--- |
//! | 1 — Nativo | SQLite (`rusqlite`), PostgreSQL / MySQL (`sqlx`), SQL Server (`tiberius`) |
//! | 2 — Ponte  | ODBC (`odbc-api`, feature `odbc`) |
//! | Arquivos / APIs | CSV, JSON (Polars + SQL em memória), REST/JSON |
//! | 3 — Sidecar | contrato gRPC em `proto/connector_plugin.proto` |

pub mod dialect;
pub mod diag;
pub mod drivers;
pub mod error;
mod gateway;
mod introspect;
pub mod net;
pub mod params;
pub mod registry;
pub mod rowset;
pub mod store;
pub mod vault;

pub use error::{ConnectorError, ConnectorResult, NetErrorKind};
pub use registry::{ConnectorRegistry, Driver, DriverInfo};

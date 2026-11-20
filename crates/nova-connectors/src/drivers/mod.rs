//! Implementações concretas de drivers.

pub mod file;
pub mod sqlite;

#[cfg(feature = "mssql")]
pub mod mssql;
#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "odbc")]
pub mod odbc;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "rest")]
pub mod rest;

#[cfg(any(feature = "postgres", feature = "mysql"))]
mod sqlx_common;

use std::time::Instant;

pub(crate) fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

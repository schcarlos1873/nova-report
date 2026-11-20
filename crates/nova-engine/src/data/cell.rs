use chrono::{DateTime, NaiveDate, NaiveDateTime};
use polars::prelude::{AnyValue, TimeUnit};
use serde::Serialize;

/// Valor escalar neutro usado na fronteira driver → DataFrame e DataFrame → expressões.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum CellValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
}

const EPOCH: NaiveDate = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();

impl CellValue {
    pub fn from_any(av: &AnyValue<'_>) -> Self {
        match av {
            AnyValue::Null => CellValue::Null,
            AnyValue::Boolean(b) => CellValue::Bool(*b),
            AnyValue::String(s) => CellValue::Text((*s).to_string()),
            AnyValue::StringOwned(s) => CellValue::Text(s.to_string()),
            AnyValue::Int8(v) => CellValue::Int(*v as i64),
            AnyValue::Int16(v) => CellValue::Int(*v as i64),
            AnyValue::Int32(v) => CellValue::Int(*v as i64),
            AnyValue::Int64(v) => CellValue::Int(*v),
            AnyValue::UInt8(v) => CellValue::Int(*v as i64),
            AnyValue::UInt16(v) => CellValue::Int(*v as i64),
            AnyValue::UInt32(v) => CellValue::Int(*v as i64),
            AnyValue::UInt64(v) => CellValue::Int(*v as i64),
            AnyValue::Float32(v) => CellValue::Float(*v as f64),
            AnyValue::Float64(v) => CellValue::Float(*v),
            AnyValue::Date(days) => CellValue::Date(EPOCH + chrono::Duration::days(*days as i64)),
            AnyValue::Datetime(v, unit, _) | AnyValue::DatetimeOwned(v, unit, _) => {
                let (secs, nanos) = match unit {
                    TimeUnit::Nanoseconds => (v.div_euclid(1_000_000_000), v.rem_euclid(1_000_000_000)),
                    TimeUnit::Microseconds => (v.div_euclid(1_000_000), v.rem_euclid(1_000_000) * 1_000),
                    TimeUnit::Milliseconds => (v.div_euclid(1_000), v.rem_euclid(1_000) * 1_000_000),
                };
                DateTime::from_timestamp(secs, nanos as u32)
                    .map(|d| CellValue::DateTime(d.naive_utc()))
                    .unwrap_or(CellValue::Null)
            }
            other => CellValue::Text(other.to_string().trim_matches('"').to_string()),
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            CellValue::Int(v) => Some(*v as f64),
            CellValue::Float(v) => Some(*v),
            CellValue::Bool(b) => Some(*b as i64 as f64),
            CellValue::Text(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, CellValue::Null)
    }

    pub fn to_display(&self) -> String {
        match self {
            CellValue::Null => String::new(),
            CellValue::Bool(b) => b.to_string(),
            CellValue::Int(v) => v.to_string(),
            CellValue::Float(v) => crate::expression::format::plain_float(*v),
            CellValue::Text(s) => s.clone(),
            CellValue::Date(d) => d.format("%Y-%m-%d").to_string(),
            CellValue::DateTime(d) => d.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

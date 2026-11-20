//! Parâmetros de relatório: tipos declarados e valores coeridos.

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::{DomainError, DomainResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    #[default]
    String,
    Int,
    Float,
    Bool,
    Date,
    DateTime,
}

/// Valor de parâmetro já tipado, pronto para bind nos drivers.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Null,
}

impl ParamValue {
    /// Converte um valor JSON vindo da UI/CLI para o tipo declarado do parâmetro.
    pub fn coerce(name: &str, ty: ParamType, raw: &serde_json::Value) -> DomainResult<Self> {
        use serde_json::Value as J;
        let err = |reason: &str| DomainError::InvalidParameter {
            name: name.to_string(),
            reason: reason.to_string(),
        };
        if raw.is_null() || raw.as_str().is_some_and(|s| s.trim().is_empty()) {
            return Ok(ParamValue::Null);
        }
        Ok(match ty {
            ParamType::String => match raw {
                J::String(s) => ParamValue::Text(s.clone()),
                other => ParamValue::Text(other.to_string()),
            },
            ParamType::Int => match raw {
                J::Number(n) => ParamValue::Int(n.as_i64().ok_or_else(|| err("esperado inteiro"))?),
                J::String(s) => ParamValue::Int(s.trim().parse().map_err(|_| err("esperado inteiro"))?),
                _ => return Err(err("esperado inteiro")),
            },
            ParamType::Float => match raw {
                J::Number(n) => ParamValue::Float(n.as_f64().ok_or_else(|| err("esperado número"))?),
                J::String(s) => ParamValue::Float(
                    s.trim().replace(',', ".").parse().map_err(|_| err("esperado número"))?,
                ),
                _ => return Err(err("esperado número")),
            },
            ParamType::Bool => match raw {
                J::Bool(b) => ParamValue::Bool(*b),
                J::String(s) => match s.trim().to_lowercase().as_str() {
                    "true" | "1" | "sim" | "s" | "yes" => ParamValue::Bool(true),
                    "false" | "0" | "nao" | "não" | "n" | "no" => ParamValue::Bool(false),
                    _ => return Err(err("esperado booleano")),
                },
                _ => return Err(err("esperado booleano")),
            },
            ParamType::Date => {
                let s = raw.as_str().ok_or_else(|| err("esperado data AAAA-MM-DD"))?;
                ParamValue::Date(parse_date(s).ok_or_else(|| err("esperado data AAAA-MM-DD"))?)
            }
            ParamType::DateTime => {
                let s = raw.as_str().ok_or_else(|| err("esperado data/hora ISO"))?;
                ParamValue::DateTime(parse_datetime(s).ok_or_else(|| err("esperado data/hora ISO"))?)
            }
        })
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            ParamValue::Int(v) => (*v).into(),
            ParamValue::Float(v) => serde_json::Number::from_f64(*v)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            ParamValue::Text(v) => v.clone().into(),
            ParamValue::Bool(v) => (*v).into(),
            ParamValue::Date(d) => d.format("%Y-%m-%d").to_string().into(),
            ParamValue::DateTime(d) => d.format("%Y-%m-%dT%H:%M:%S").to_string().into(),
            ParamValue::Null => serde_json::Value::Null,
        }
    }

    /// Representação textual (usada por providers baseados em texto, ex.: REST/gRPC).
    pub fn to_text(&self) -> String {
        match self {
            ParamValue::Text(s) => s.clone(),
            ParamValue::Null => String::new(),
            other => other.to_json().to_string().trim_matches('"').to_string(),
        }
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(s, "%d/%m/%Y"))
        .ok()
        .or_else(|| parse_datetime(s).map(|dt| dt.date()))
}

fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(s, f).ok())
        .or_else(|| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn coerces_dates_in_iso_and_br_formats() {
        let iso = ParamValue::coerce("d", ParamType::Date, &json!("2024-03-01")).unwrap();
        let br = ParamValue::coerce("d", ParamType::Date, &json!("01/03/2024")).unwrap();
        assert_eq!(iso, br);
    }

    #[test]
    fn empty_string_is_null_and_bad_number_fails() {
        assert_eq!(ParamValue::coerce("x", ParamType::Int, &json!("")).unwrap(), ParamValue::Null);
        assert!(ParamValue::coerce("x", ParamType::Int, &json!("abc")).is_err());
        assert_eq!(
            ParamValue::coerce("x", ParamType::Float, &json!("10,5")).unwrap(),
            ParamValue::Float(10.5)
        );
    }
}

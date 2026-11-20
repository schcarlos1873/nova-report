//! Coerção e validação dos parâmetros informados pelo usuário.

use std::collections::HashMap;

use nova_domain::report::ReportDefinition;
use nova_domain::{DomainError, ParamType, ParamValue};

use crate::AppResult;

/// Converte os valores JSON da UI/CLI nos tipos declarados, aplicando defaults.
/// Defaults especiais: `"today"`, `"firstDayOfMonth"`, `"firstDayOfYear"` para datas.
pub fn resolve(def: &ReportDefinition, raw: &HashMap<String, serde_json::Value>) -> AppResult<HashMap<String, ParamValue>> {
    let mut out = HashMap::new();
    for p in &def.parameters {
        let given = raw.get(&p.name).filter(|v| !v.is_null() && v.as_str().is_none_or(|s| !s.trim().is_empty()));
        let value = match (given, &p.default_value) {
            (Some(v), _) => ParamValue::coerce(&p.name, p.param_type, v)?,
            (None, Some(d)) => ParamValue::coerce(&p.name, p.param_type, &dynamic_default(p.param_type, d))?,
            (None, None) => ParamValue::Null,
        };
        if p.required && value == ParamValue::Null {
            return Err(DomainError::MissingParameter(if p.label.is_empty() { p.name.clone() } else { p.label.clone() }).into());
        }
        out.insert(p.name.clone(), value);
    }
    Ok(out)
}

fn dynamic_default(ty: ParamType, d: &serde_json::Value) -> serde_json::Value {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    let date = match (ty, d.as_str()) {
        (ParamType::Date | ParamType::DateTime, Some("today")) => Some(today),
        (ParamType::Date | ParamType::DateTime, Some("firstDayOfMonth")) => today.with_day(1),
        (ParamType::Date | ParamType::DateTime, Some("firstDayOfYear")) => today.with_ordinal(1),
        _ => None,
    };
    date.map(|x| serde_json::Value::String(x.format("%Y-%m-%d").to_string())).unwrap_or_else(|| d.clone())
}

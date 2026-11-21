//! Coerção e validação dos parâmetros informados pelo usuário.

use std::collections::HashMap;

use nova_domain::report::{ParameterDef, ReportDefinition};
use nova_domain::{DomainError, ParamType, ParamValue};

use crate::AppResult;

/// Converte os valores JSON da UI/CLI nos tipos declarados, aplicando defaults.
/// Defaults especiais: `"today"`, `"firstDayOfMonth"`, `"firstDayOfYear"` para datas.
/// Parâmetros de vários valores viram `ParamValue::List` (array JSON ou texto separado por vírgula).
pub fn resolve(def: &ReportDefinition, raw: &HashMap<String, serde_json::Value>) -> AppResult<HashMap<String, ParamValue>> {
    let mut out = HashMap::new();
    for p in &def.parameters {
        let value = resolve_one(p, raw.get(&p.name))?;
        if p.required && value.is_empty() {
            return Err(DomainError::MissingParameter(if p.label.is_empty() { p.name.clone() } else { p.label.clone() }).into());
        }
        out.insert(p.name.clone(), value);
    }
    Ok(out)
}

/// Como `resolve`, mas sem falhar: valores inválidos ou obrigatórios ausentes ficam vazios.
/// Usada no designer (descobrir campos) e nas listas em cascata, onde parte dos parâmetros
/// ainda não foi preenchida — os que foram continuam valendo.
pub fn resolve_lenient(def: &ReportDefinition, raw: &HashMap<String, serde_json::Value>) -> HashMap<String, ParamValue> {
    def.parameters
        .iter()
        .map(|p| {
            let empty = if p.multi_value { ParamValue::List(vec![]) } else { ParamValue::Null };
            (p.name.clone(), resolve_one(p, raw.get(&p.name)).unwrap_or(empty))
        })
        .collect()
}

fn resolve_one(p: &ParameterDef, given: Option<&serde_json::Value>) -> AppResult<ParamValue> {
    let coerce = |v: &serde_json::Value| {
        if p.multi_value { ParamValue::coerce_many(&p.name, p.param_type, v) } else { ParamValue::coerce(&p.name, p.param_type, v) }
    };
    // array vazio é uma escolha explícita ("nenhum"), não ausência de valor
    let given = given.filter(|v| !v.is_null() && v.as_str().is_none_or(|s| !s.trim().is_empty()));
    Ok(match (given, &p.default_value) {
        (Some(v), _) => coerce(v)?,
        (None, Some(d)) if p.multi_value => match d {
            serde_json::Value::Array(items) => coerce(&serde_json::Value::Array(items.iter().map(|x| dynamic_default(p.param_type, x)).collect()))?,
            other => coerce(&dynamic_default(p.param_type, other))?,
        },
        (None, Some(d)) => coerce(&dynamic_default(p.param_type, d))?,
        (None, None) if p.multi_value => ParamValue::List(vec![]),
        (None, None) => ParamValue::Null,
    })
}

/// Troca cada valor escolhido pelo rótulo da lista de `availableValues` (o próprio valor em texto
/// quando ele não está na lista), mantendo o formato: texto para valor simples, lista para vários.
pub fn labels_for(value: &ParamValue, options: &[crate::ParameterOption]) -> ParamValue {
    match value {
        ParamValue::Null => ParamValue::Null,
        ParamValue::List(v) => ParamValue::List(v.iter().map(|x| labels_for(x, options)).collect()),
        one => {
            let json = one.to_json();
            ParamValue::Text(options.iter().find(|o| o.value == json).map_or_else(|| one.to_text(), |o| o.label.clone()))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn def() -> ReportDefinition {
        let mut d = ReportDefinition::new_blank("t");
        d.parameters = serde_json::from_value(json!([
            { "name": "fab", "type": "int", "required": true },
            { "name": "maq", "type": "int", "multiValue": true, "required": true, "defaultValue": [1, 2] },
            { "name": "st", "type": "string", "multiValue": true }
        ]))
        .unwrap();
        d
    }

    #[test]
    fn resolves_multi_value_with_defaults() {
        let raw = HashMap::from([("fab".to_string(), json!("3")), ("st".to_string(), json!("A,B"))]);
        let p = resolve(&def(), &raw).unwrap();
        assert_eq!(p["maq"], ParamValue::List(vec![ParamValue::Int(1), ParamValue::Int(2)]));
        assert_eq!(p["st"], ParamValue::List(vec![ParamValue::Text("A".into()), ParamValue::Text("B".into())]));
    }

    #[test]
    fn required_multi_value_rejects_empty_selection() {
        let raw = HashMap::from([("fab".to_string(), json!(3)), ("maq".to_string(), json!([]))]);
        assert!(resolve(&def(), &raw).is_err());
    }

    #[test]
    fn lenient_keeps_filled_parameters() {
        // `fab` (obrigatório) inválido não apaga `st`
        let raw = HashMap::from([("fab".to_string(), json!("x")), ("st".to_string(), json!(["A"]))]);
        let p = resolve_lenient(&def(), &raw);
        assert_eq!(p["fab"], ParamValue::Null);
        assert_eq!(p["st"], ParamValue::List(vec![ParamValue::Text("A".into())]));
    }

    #[test]
    fn labels_follow_value_shape() {
        let opts = vec![
            crate::ParameterOption { value: json!(1), label: "Norte".into() },
            crate::ParameterOption { value: json!(2), label: "Sul".into() },
        ];
        assert_eq!(labels_for(&ParamValue::Int(2), &opts), ParamValue::Text("Sul".into()));
        assert_eq!(
            labels_for(&ParamValue::List(vec![ParamValue::Int(1), ParamValue::Int(9)]), &opts),
            ParamValue::List(vec![ParamValue::Text("Norte".into()), ParamValue::Text("9".into())])
        );
        assert_eq!(labels_for(&ParamValue::Null, &opts), ParamValue::Null);
    }
}

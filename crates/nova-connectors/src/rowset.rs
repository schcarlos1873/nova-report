//! Conversão de linhas genéricas (vindas de qualquer driver) para `DataFrame`,
//! com inferência do tipo de coluna.

use chrono::{NaiveDate, NaiveDateTime};
use nova_engine::data::{CellValue, ColumnKind, DataFrameBuilder};
use polars::prelude::DataFrame;

use crate::ConnectorResult;

#[derive(Debug, Default)]
pub struct RowSet {
    /// (nome, tipo nativo opcional)
    pub columns: Vec<(String, Option<String>)>,
    pub rows: Vec<Vec<CellValue>>,
}

impl RowSet {
    pub fn new(columns: Vec<(String, Option<String>)>) -> Self {
        Self { columns, rows: Vec::new() }
    }

    pub fn into_dataframe(self) -> ConnectorResult<DataFrame> {
        let RowSet { columns, mut rows } = self;
        let mut schema = Vec::with_capacity(columns.len());
        let mut used = std::collections::HashSet::new();
        for (i, (name, native)) in columns.iter().enumerate() {
            let kind = infer_kind(rows.iter().map(|r| r.get(i).unwrap_or(&CellValue::Null)), native.as_deref());
            if matches!(kind, ColumnKind::Date | ColumnKind::DateTime) {
                for r in rows.iter_mut() {
                    if let Some(CellValue::Text(s)) = r.get(i) {
                        let parsed = parse_temporal(s);
                        r[i] = parsed.unwrap_or(CellValue::Null);
                    }
                }
            }
            // nomes duplicados (ex.: joins) recebem sufixo
            let mut unique = if name.is_empty() { format!("col_{}", i + 1) } else { name.clone() };
            let mut n = 2;
            while !used.insert(unique.clone()) {
                unique = format!("{name}_{n}");
                n += 1;
            }
            schema.push((unique, kind));
        }
        let mut b = DataFrameBuilder::new(schema);
        for r in rows {
            b.push_row(r);
        }
        Ok(b.finish()?)
    }
}

fn parse_temporal(s: &str) -> Option<CellValue> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(CellValue::Date(d));
    }
    ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(s, f).ok())
        .map(CellValue::DateTime)
}

fn infer_kind<'a>(values: impl Iterator<Item = &'a CellValue>, native: Option<&str>) -> ColumnKind {
    let (mut ints, mut floats, mut bools, mut dates, mut dts, mut texts, mut text_dates, mut text_dts) = (0, 0, 0, 0, 0, 0, 0, 0);
    for v in values {
        match v {
            CellValue::Null => {}
            CellValue::Int(_) => ints += 1,
            CellValue::Float(_) => floats += 1,
            CellValue::Bool(_) => bools += 1,
            CellValue::Date(_) => dates += 1,
            CellValue::DateTime(_) => dts += 1,
            CellValue::Text(s) => {
                texts += 1;
                match parse_temporal(s) {
                    Some(CellValue::Date(_)) => text_dates += 1,
                    Some(CellValue::DateTime(_)) => text_dts += 1,
                    _ => {}
                }
            }
        }
    }
    let native = native.unwrap_or("").to_ascii_uppercase();
    let declared_temporal = native.contains("DATE") || native.contains("TIME");
    if texts > 0 {
        if declared_temporal && text_dates + text_dts == texts {
            return if text_dts > 0 || dts > 0 { ColumnKind::DateTime } else { ColumnKind::Date };
        }
        return ColumnKind::Text;
    }
    match (ints, floats, bools, dates, dts) {
        (0, 0, 0, 0, 0) if declared_temporal => ColumnKind::Date,
        (0, 0, 0, 0, 0) => match native.as_str() {
            n if n.contains("INT") => ColumnKind::Int,
            n if ["REAL", "FLOA", "DOUB", "NUMERIC", "DECIMAL", "MONEY"].iter().any(|k| n.contains(k)) => ColumnKind::Float,
            _ => ColumnKind::Text,
        },
        (_, 0, 0, 0, 0) => ColumnKind::Int,
        (_, _, 0, 0, 0) => ColumnKind::Float,
        (0, 0, _, 0, 0) => ColumnKind::Bool,
        (0, 0, 0, _, 0) => ColumnKind::Date,
        (0, 0, 0, _, _) => ColumnKind::DateTime,
        _ => ColumnKind::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_mixed_numeric_as_float_and_text_dates() {
        let mut rs = RowSet::new(vec![("v".into(), None), ("d".into(), Some("DATE".into())), ("v".into(), None)]);
        rs.rows.push(vec![CellValue::Int(1), CellValue::Text("2024-01-02".into()), CellValue::Null]);
        rs.rows.push(vec![CellValue::Float(2.5), CellValue::Null, CellValue::Text("x".into())]);
        let df = rs.into_dataframe().unwrap();
        assert_eq!(df.column("v").unwrap().dtype(), &polars::prelude::DataType::Float64);
        assert_eq!(df.column("d").unwrap().dtype(), &polars::prelude::DataType::Date);
        assert!(df.column("v_2").is_ok());
    }
}

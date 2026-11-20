//! Transformações em memória: joins heterogêneos e matrizes (cross-tab).

use std::collections::BTreeMap;

use polars::prelude::*;

use super::{CellValue, ColumnKind, DataFrameBuilder};
use crate::EngineResult;

/// Left join entre datasets possivelmente vindos de bancos diferentes.
pub fn join_datasets(left: DataFrame, right: DataFrame, left_key: &str, right_key: &str) -> EngineResult<DataFrame> {
    Ok(left
        .lazy()
        .join(right.lazy(), [col(left_key)], [col(right_key)], JoinArgs::new(JoinType::Left))
        .collect()?)
}

/// Pivota `row_field` × `col_field` somando `value_field`, com totais por linha e coluna.
pub fn crosstab(df: &DataFrame, row_field: &str, col_field: &str, value_field: &str) -> EngineResult<DataFrame> {
    let rows = df.column(row_field)?;
    let cols = df.column(col_field)?;
    let vals = df.column(value_field)?;
    let mut matrix: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    let mut col_keys = std::collections::BTreeSet::new();
    for i in 0..df.height() {
        let r = CellValue::from_any(&rows.get(i)?).to_display();
        let c = CellValue::from_any(&cols.get(i)?).to_display();
        let v = CellValue::from_any(&vals.get(i)?).as_f64().unwrap_or(0.0);
        col_keys.insert(c.clone());
        *matrix.entry(r).or_default().entry(c).or_default() += v;
    }
    let mut schema = vec![(row_field.to_string(), ColumnKind::Text)];
    schema.extend(col_keys.iter().map(|c| (c.clone(), ColumnKind::Float)));
    schema.push(("Total".into(), ColumnKind::Float));
    let mut builder = DataFrameBuilder::new(schema);
    let mut col_totals = vec![0.0; col_keys.len()];
    for (r, cells) in &matrix {
        let mut row = vec![CellValue::Text(r.clone())];
        let mut total = 0.0;
        for (i, c) in col_keys.iter().enumerate() {
            let v = cells.get(c).copied().unwrap_or(0.0);
            col_totals[i] += v;
            total += v;
            row.push(CellValue::Float(v));
        }
        row.push(CellValue::Float(total));
        builder.push_row(row);
    }
    let grand: f64 = col_totals.iter().sum();
    let mut footer = vec![CellValue::Text("Total".into())];
    footer.extend(col_totals.into_iter().map(CellValue::Float));
    footer.push(CellValue::Float(grand));
    builder.push_row(footer);
    builder.finish()
}

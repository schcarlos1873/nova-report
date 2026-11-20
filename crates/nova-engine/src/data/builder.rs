use chrono::NaiveDate;
use polars::prelude::*;

use super::CellValue;
use crate::EngineResult;

const EPOCH: NaiveDate = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    Bool,
    Int,
    Float,
    Text,
    Date,
    DateTime,
}

/// Constrói um `DataFrame` coluna a coluna a partir de linhas vindas de drivers.
pub struct DataFrameBuilder {
    names: Vec<String>,
    kinds: Vec<ColumnKind>,
    cols: Vec<Vec<CellValue>>,
}

impl DataFrameBuilder {
    pub fn new(columns: Vec<(String, ColumnKind)>) -> Self {
        let n = columns.len();
        let (names, kinds) = columns.into_iter().unzip();
        Self { names, kinds, cols: vec![Vec::new(); n] }
    }

    pub fn push_row(&mut self, row: Vec<CellValue>) {
        for (i, v) in row.into_iter().enumerate().take(self.cols.len()) {
            self.cols[i].push(v);
        }
    }

    pub fn finish(self) -> EngineResult<DataFrame> {
        let height = self.cols.first().map_or(0, |c| c.len());
        let mut columns = Vec::with_capacity(self.names.len());
        for ((name, kind), values) in self.names.into_iter().zip(self.kinds).zip(self.cols) {
            let name: PlSmallStr = name.into();
            let col = match kind {
                ColumnKind::Bool => Column::new(
                    name,
                    values.iter().map(|v| match v { CellValue::Bool(b) => Some(*b), _ => None }).collect::<Vec<_>>(),
                ),
                ColumnKind::Int => Column::new(
                    name,
                    values.iter().map(|v| match v {
                        CellValue::Int(i) => Some(*i),
                        CellValue::Float(f) => Some(*f as i64),
                        CellValue::Bool(b) => Some(*b as i64),
                        _ => None,
                    }).collect::<Vec<_>>(),
                ),
                ColumnKind::Float => Column::new(name, values.iter().map(CellValue::as_f64).collect::<Vec<_>>()),
                ColumnKind::Text => Column::new(
                    name,
                    values.iter().map(|v| if v.is_null() { None } else { Some(v.to_display()) }).collect::<Vec<_>>(),
                ),
                ColumnKind::Date => Column::new(
                    name,
                    values.iter().map(|v| match v {
                        CellValue::Date(d) => Some((*d - EPOCH).num_days() as i32),
                        CellValue::DateTime(d) => Some((d.date() - EPOCH).num_days() as i32),
                        _ => None,
                    }).collect::<Vec<_>>(),
                )
                .cast(&DataType::Date)?,
                ColumnKind::DateTime => Column::new(
                    name,
                    values.iter().map(|v| match v {
                        CellValue::DateTime(d) => Some(d.and_utc().timestamp_millis()),
                        CellValue::Date(d) => d.and_hms_opt(0, 0, 0).map(|x| x.and_utc().timestamp_millis()),
                        _ => None,
                    }).collect::<Vec<_>>(),
                )
                .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))?,
            };
            columns.push(col);
        }
        Ok(DataFrame::new(height, columns)?)
    }
}

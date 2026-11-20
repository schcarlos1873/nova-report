//! Exportadores de dados: CSV e XLSX com tipos numéricos nativos e fórmulas de total.

use polars::prelude::*;
use rust_xlsxwriter::{Format, Formula, Workbook};

use crate::data::CellValue;
use crate::{EngineError, EngineResult};

/// CSV (RFC 4180) com cabeçalho. `separator` costuma ser `,` ou `;` (Excel pt-BR).
pub fn to_csv(df: &DataFrame, separator: u8) -> EngineResult<Vec<u8>> {
    let mut out = Vec::new();
    let sep = separator as char;
    let quote = |s: &str| {
        if s.contains(sep) || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let names: Vec<String> = df.get_column_names().iter().map(|n| quote(n.as_str())).collect();
    out.extend(names.join(&sep.to_string()).into_bytes());
    out.extend(b"\r\n");
    for r in 0..df.height() {
        let mut cells = Vec::with_capacity(df.width());
        for c in df.columns() {
            cells.push(quote(&CellValue::from_any(&c.get(r)?).to_display()));
        }
        out.extend(cells.join(&sep.to_string()).into_bytes());
        out.extend(b"\r\n");
    }
    Ok(out)
}

/// Uma planilha por dataset. Números ficam numéricos, datas como datas Excel e uma
/// linha final com `=SUM(...)` para cada coluna numérica.
pub fn to_xlsx(sheets: &[(String, &DataFrame)]) -> EngineResult<Vec<u8>> {
    let err = |e: rust_xlsxwriter::XlsxError| EngineError::Export(e.to_string());
    let mut wb = Workbook::new();
    let header = Format::new().set_bold().set_background_color("E2E8F0");
    let total_fmt = Format::new().set_bold().set_num_format("#,##0.00");
    let num_fmt = Format::new().set_num_format("#,##0.00");
    let date_fmt = Format::new().set_num_format("dd/mm/yyyy");
    let dt_fmt = Format::new().set_num_format("dd/mm/yyyy hh:mm");

    for (name, df) in sheets {
        let ws = wb.add_worksheet();
        let safe: String = name.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
        ws.set_name(if safe.is_empty() { "Dados" } else { &safe }).map_err(err)?;
        for (ci, col) in df.columns().iter().enumerate() {
            let ci = ci as u16;
            ws.write_string_with_format(0, ci, col.name().as_str(), &header).map_err(err)?;
            ws.set_column_width(ci, 16).map_err(err)?;
            let numeric = col.dtype().is_primitive_numeric();
            for r in 0..df.height() {
                let row = (r + 1) as u32;
                match CellValue::from_any(&col.get(r)?) {
                    CellValue::Null => {}
                    CellValue::Int(v) => {
                        ws.write_number(row, ci, v as f64).map_err(err)?;
                    }
                    CellValue::Float(v) => {
                        ws.write_number_with_format(row, ci, v, &num_fmt).map_err(err)?;
                    }
                    CellValue::Bool(b) => {
                        ws.write_boolean(row, ci, b).map_err(err)?;
                    }
                    CellValue::Date(d) => {
                        let dt = rust_xlsxwriter::ExcelDateTime::parse_from_str(&d.format("%Y-%m-%d").to_string()).map_err(err)?;
                        ws.write_datetime_with_format(row, ci, &dt, &date_fmt).map_err(err)?;
                    }
                    CellValue::DateTime(d) => {
                        let dt = rust_xlsxwriter::ExcelDateTime::parse_from_str(&d.format("%Y-%m-%dT%H:%M:%S").to_string()).map_err(err)?;
                        ws.write_datetime_with_format(row, ci, &dt, &dt_fmt).map_err(err)?;
                    }
                    CellValue::Text(s) => {
                        ws.write_string(row, ci, &s).map_err(err)?;
                    }
                }
            }
            if numeric && df.height() > 0 {
                let letter = column_letter(ci);
                let formula = Formula::new(format!("=SUM({letter}2:{letter}{})", df.height() + 1));
                ws.write_formula_with_format((df.height() + 1) as u32, ci, formula, &total_fmt).map_err(err)?;
            }
        }
        ws.set_freeze_panes(1, 0).map_err(err)?;
        if df.width() > 0 {
            ws.autofilter(0, 0, df.height() as u32, (df.width() - 1) as u16).map_err(err)?;
        }
    }
    if sheets.is_empty() {
        wb.add_worksheet();
    }
    wb.save_to_buffer().map_err(err)
}

fn column_letter(mut idx: u16) -> String {
    let mut s = Vec::new();
    loop {
        s.push((b'A' + (idx % 26) as u8) as char);
        if idx < 26 {
            break;
        }
        idx = idx / 26 - 1;
    }
    s.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{ColumnKind, DataFrameBuilder};

    #[test]
    fn excel_column_letters() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
        assert_eq!(column_letter(701), "ZZ");
    }

    #[test]
    fn csv_quotes_special_chars_and_xlsx_is_zip() {
        let mut b = DataFrameBuilder::new(vec![("nome".into(), ColumnKind::Text), ("v".into(), ColumnKind::Float)]);
        b.push_row(vec![CellValue::Text("a;b".into()), CellValue::Float(1.5)]);
        let df = b.finish().unwrap();
        let csv = String::from_utf8(to_csv(&df, b';').unwrap()).unwrap();
        assert_eq!(csv, "nome;v\r\n\"a;b\";1.5\r\n");
        let xlsx = to_xlsx(&[("Vendas".into(), &df)]).unwrap();
        assert_eq!(&xlsx[..2], b"PK");
    }
}

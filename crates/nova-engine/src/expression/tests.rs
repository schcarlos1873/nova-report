use std::collections::HashMap;

use nova_domain::ParamValue;

use super::*;
use crate::data::{CellValue, ColumnKind, DataFrameBuilder};

fn sample() -> DataFrame {
    let mut b = DataFrameBuilder::new(vec![
        ("cliente".into(), ColumnKind::Text),
        ("valor".into(), ColumnKind::Float),
        ("qtd".into(), ColumnKind::Int),
    ]);
    b.push_row(vec![CellValue::Text("Ana".into()), CellValue::Float(10.5), CellValue::Int(1)]);
    b.push_row(vec![CellValue::Text("Bruno".into()), CellValue::Float(20.0), CellValue::Int(2)]);
    b.push_row(vec![CellValue::Text("Ana".into()), CellValue::Float(-5.0), CellValue::Int(3)]);
    b.finish().unwrap()
}

fn engine() -> FormulaEngine {
    let mut params = HashMap::new();
    params.insert("minimo".into(), ParamValue::Float(0.0));
    let mut e = FormulaEngine::new(&params, "Teste");
    e.set_dataset(Some(sample()));
    e
}

#[test]
fn fields_and_spec_format_namespace() {
    let mut e = engine();
    e.set_row(Some(0));
    assert_eq!(e.eval_string("Fields.cliente").unwrap(), "Ana");
    assert_eq!(e.eval_string("Format.currency(Fields.valor, 'BRL')").unwrap(), "R$ 10,50");
    assert_eq!(e.eval_string("Fields.qtd * 2.5").unwrap(), "2.5");
}

#[test]
fn aggregates_follow_current_range() {
    let mut e = engine();
    assert_eq!(e.eval_string("SUM(\"valor\")").unwrap(), "25.5");
    assert_eq!(e.eval_string("Group.Sum_valor").unwrap(), "25.5");
    e.set_range(1, 2);
    assert_eq!(e.eval_string("SUM(\"valor\")").unwrap(), "15");
    assert_eq!(e.eval_string("COUNT()").unwrap(), "2");
    assert_eq!(e.eval_string("DISTINCT_COUNT(\"cliente\")").unwrap(), "2");
    e.set_row(Some(1));
    assert_eq!(e.eval_string("RUNNING_SUM(\"valor\")").unwrap(), "20");
    assert_eq!(e.eval_string("RECORD_NUMBER()").unwrap(), "1");
}

#[test]
fn min_max_handle_numbers_and_dates() {
    let d = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    let dt = |s: &str| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap();
    let mut b = DataFrameBuilder::new(vec![
        ("inicio".into(), ColumnKind::DateTime),
        ("dia".into(), ColumnKind::Date),
        ("peso".into(), ColumnKind::Float),
    ]);
    b.push_row(vec![CellValue::DateTime(dt("2026-10-02 08:30:00")), CellValue::Date(d("2026-10-02")), CellValue::Float(3.0)]);
    b.push_row(vec![CellValue::DateTime(dt("2026-10-01 22:15:00")), CellValue::Null, CellValue::Float(-1.5)]);
    b.push_row(vec![CellValue::DateTime(dt("2026-10-04 06:00:00")), CellValue::Date(d("2026-10-03")), CellValue::Float(7.0)]);
    let mut e = FormulaEngine::new(&HashMap::new(), "Teste");
    e.set_dataset(Some(b.finish().unwrap()));

    assert_eq!(e.eval_string("MIN(\"peso\")").unwrap(), "-1.5");
    assert_eq!(e.eval_string("MAX(\"peso\")").unwrap(), "7");
    assert_eq!(e.eval_string("MIN(\"inicio\")").unwrap(), "2026-10-01 22:15:00");
    assert_eq!(e.eval_string("MAX(\"inicio\")").unwrap(), "2026-10-04 06:00:00");
    assert_eq!(e.eval_string("MIN(\"dia\")").unwrap(), "2026-10-02");
    assert_eq!(e.eval_string("FORMAT_DATE(MAX(\"inicio\"), \"%d/%m/%Y %H:%M\")").unwrap(), "04/10/2026 06:00");
    e.set_range(0, 2);
    assert_eq!(e.eval_string("MAX(\"inicio\")").unwrap(), "2026-10-02 08:30:00");
}

#[test]
fn conditionals_params_and_sandbox() {
    let mut e = engine();
    e.set_row(Some(2));
    assert_eq!(e.eval_string("IIF(Fields.valor < Params.minimo, \"#DC2626\", \"#000000\")").unwrap(), "#DC2626");
    assert!(!e.eval_bool("Fields.valor > 0").unwrap());
    // loop infinito é interrompido pelo limite de operações
    assert!(e.eval("loop { }").is_err());
    assert!(e.check("1 +").is_err());
}

#[test]
fn preprocess_single_quotes() {
    assert_eq!(preprocess("f('BRL', \"it's\")"), "f(\"BRL\", \"it's\")");
    assert_eq!(preprocess("'d''x'"), "\"d'x\"");
}

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use nova_domain::report::*;

use super::*;
use crate::data::{CellValue, ColumnKind, DataContext, DataFrameBuilder};
use crate::render::{FontLibrary, ReportCompiler, RenderedElement};

static FONTS: LazyLock<Arc<FontLibrary>> = LazyLock::new(|| Arc::new(FontLibrary::load(false)));

fn report() -> ReportDefinition {
    let json = r##"{
      "meta": { "title": "Vendas por Cliente", "paperSize": "A4" },
      "dataSources": [{ "id": "ds", "provider": "sqlite", "options": {"path": "x.db"},
                        "queries": [{ "name": "Pedidos", "sql": "SELECT * FROM pedidos" }] }],
      "groups": [{ "name": "cli", "expression": "Fields.cliente" }],
      "bands": [
        { "type": "pageHeader", "height": 12, "elements": [
          { "id": "t", "type": "label", "text": "Vendas", "x": 0, "y": 0, "width": 100, "height": 8 } ]},
        { "type": "groupHeader", "group": "cli", "height": 8, "elements": [
          { "id": "gh", "type": "expression", "expression": "\"Cliente: \" + Fields.cliente", "x": 0, "y": 0, "width": 100, "height": 8 } ]},
        { "type": "detail", "height": 6, "dataSource": "Pedidos", "elements": [
          { "id": "v", "type": "expression", "expression": "Fields.valor", "format": "currency:BRL",
            "colorExpression": "IIF(Fields.valor < 0, '#DC2626', '#000000')",
            "x": 100, "y": 0, "width": 40, "height": 6 } ]},
        { "type": "groupFooter", "group": "cli", "height": 8, "elements": [
          { "id": "gf", "type": "expression", "expression": "SUM(\"valor\")", "format": "number:2", "x": 100, "y": 0, "width": 40, "height": 8 } ]},
        { "type": "pageFooter", "height": 8, "elements": [
          { "id": "pg", "type": "label", "text": "Página {PAGE} de {PAGES}", "x": 0, "y": 0, "width": 60, "height": 6 } ]},
        { "type": "reportFooter", "height": 30, "elements": [
          { "id": "tot", "type": "expression", "expression": "Format.currency(SUM('valor'), 'BRL')", "x": 0, "y": 0, "width": 60, "height": 8 },
          { "id": "tb", "type": "table", "dataSource": "Pedidos", "x": 0, "y": 10, "width": 120, "height": 20,
            "zebraBackground": "#F8FAFC",
            "columns": [
              { "header": "Cliente", "expression": "Fields.cliente", "width": 60 },
              { "header": "Valor", "expression": "Fields.valor", "width": 40, "align": "right", "format": "number:2", "footer": "SUM(\"valor\")" }
            ] }
        ]}
      ]
    }"##;
    ReportDefinition::from_json(json).unwrap()
}

fn data(rows: usize) -> DataContext {
    let mut b = DataFrameBuilder::new(vec![("cliente".into(), ColumnKind::Text), ("valor".into(), ColumnKind::Float)]);
    for i in 0..rows {
        let cli = ["Ana", "Bruno", "Carla"][i * 3 / rows.max(1)];
        b.push_row(vec![CellValue::Text(cli.into()), CellValue::Float(if i == 1 { -10.0 } else { 100.0 })]);
    }
    let mut datasets = HashMap::new();
    datasets.insert("Pedidos".to_string(), b.finish().unwrap());
    DataContext { datasets, ..Default::default() }
}

fn texts(band: &crate::render::RenderedBand) -> Vec<String> {
    band.elements
        .iter()
        .filter_map(|e| match e {
            RenderedElement::TextBox { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn group_breaks_and_aggregates() {
    let def = report();
    assert!(def.ensure_valid().is_ok(), "{:?}", def.validate());
    let ctx = data(6);
    let out = ReportProcessor::new(&def, &ctx, None).process().unwrap();
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let seq: Vec<_> = out.flow_bands.iter().map(|b| b.source_band_id.clone()).collect();
    assert_eq!(
        seq,
        vec![
            "group_header_1", "detail_1", "detail_1", "group_footer_1",
            "group_header_1", "detail_1", "detail_1", "group_footer_1",
            "group_header_1", "detail_1", "detail_1", "group_footer_1",
            "report_footer_1",
        ]
    );
    assert_eq!(texts(&out.flow_bands[0]), vec!["Cliente: Ana"]);
    assert_eq!(texts(&out.flow_bands[3]), vec!["90,00"]);
    assert_eq!(texts(&out.flow_bands[12])[0], "R$ 490,00");
    let RenderedElement::TextBox { color_hex, .. } = &out.flow_bands[2].elements[0] else { panic!() };
    assert_eq!(color_hex, "#DC2626");
}

#[test]
fn compiles_to_multi_page_pdf_and_svg() {
    let def = report();
    let ctx = data(240);
    let rendered = ReportProcessor::new(&def, &ctx, None).process().unwrap();
    let compiler = ReportCompiler::new(FONTS.clone());
    let compiled = compiler.compile(&rendered).unwrap_or_else(|e| panic!("{e}"));
    assert!(compiled.page_count() > 3, "páginas: {}", compiled.page_count());
    let pdf = compiled.to_pdf().unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let svg = compiled.page_svg(0).unwrap();
    assert!(svg.starts_with("<svg"));
}

/// Critério de aceite da Fase 1: preview < 50 ms para relatórios de até 50 páginas
/// (layout + Typst com caches quentes, como no designer). Rode com:
/// `cargo test -p nova-engine --release -- --ignored --nocapture perf_`
#[test]
#[ignore]
fn perf_50_pages_warm_preview() {
    let def = report();
    let ctx = data(2_000);
    let compiler = ReportCompiler::new(FONTS.clone());
    let mut pages = 0;
    for i in 0..4 {
        let t = std::time::Instant::now();
        let rendered = ReportProcessor::new(&def, &ctx, None).process().unwrap();
        let layout = t.elapsed();
        let compiled = compiler.compile(&rendered).unwrap();
        let svg_start = std::time::Instant::now();
        let first_page = compiled.page_svg(0).unwrap();
        pages = compiled.page_count();
        println!(
            "rodada {i}: {pages} páginas · layout {:?} · typst {} ms · svg pág.1 {:?} ({} KB) · total {:?}",
            layout,
            compiled.compile_ms,
            svg_start.elapsed(),
            first_page.len() / 1024,
            t.elapsed()
        );
    }
    assert!(pages >= 40, "{pages}");
}

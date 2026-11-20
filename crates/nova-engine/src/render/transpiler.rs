//! Traduz o [`RenderedReport`] em código Typst.
//!
//! Todo texto de usuário é emitido como **literal de string Typst** (nunca como markup),
//! eliminando qualquer possibilidade de injeção de código no documento.

use std::fmt::Write;

use super::model::{PageConfig, RenderedBand, RenderedElement, RenderedReport, TableCell};
use crate::expression::{PAGE_NUMBER_MARK, TOTAL_PAGES_MARK};

const FALLBACK_FONTS: [&str; 5] = ["Arial", "Helvetica", "Liberation Sans", "DejaVu Sans", "Libertinus Serif"];

pub struct TypstTranspiler;

impl TypstTranspiler {
    pub fn generate_source(report: &RenderedReport) -> String {
        let mut code = String::with_capacity(64 * 1024 + report.flow_bands.len() * 512);
        // Ordem clássica (Crystal/Jasper): na 1ª página o Report Header vem antes do Page Header.
        let title_first = report.page_header.is_some()
            && report.flow_bands.first().is_some_and(|b| b.band_type == "reportHeader");
        Self::preamble(&mut code, &report.title, &report.page, report.page_header.as_ref(), report.page_footer.as_ref(), title_first);
        for (i, band) in report.flow_bands.iter().enumerate() {
            if i == 0 && title_first {
                let ph = report.page_header.as_ref().expect("verificado acima");
                writeln!(code, "#v(-{})", mm(ph.height_mm)).unwrap();
                code.push('#');
                Self::render_band(&mut code, band);
                code.push_str("\n#");
                Self::render_band(&mut code, ph);
                code.push('\n');
                continue;
            }
            if band.page_break_before {
                code.push_str("#pagebreak(weak: true)\n");
            }
            let tables: Vec<_> = band.elements.iter().filter(|e| matches!(e, RenderedElement::Table { .. })).collect();
            if tables.is_empty() {
                code.push('#');
                Self::render_band(&mut code, band);
                code.push('\n');
            } else {
                // elementos posicionados acima da primeira tabela formam um bloco fixo
                let first_y = tables
                    .iter()
                    .filter_map(|t| match t {
                        RenderedElement::Table { y_mm, .. } => Some(*y_mm),
                        _ => None,
                    })
                    .fold(f64::MAX, f64::min);
                let statics: Vec<RenderedElement> =
                    band.elements.iter().filter(|e| !matches!(e, RenderedElement::Table { .. })).cloned().collect();
                if first_y > 0.0 {
                    let head = RenderedBand { height_mm: first_y, elements: statics, ..band.clone() };
                    code.push('#');
                    Self::render_band(&mut code, &head);
                    code.push('\n');
                }
                for t in tables {
                    Self::render_table(&mut code, t);
                }
            }
            if band.page_break_after {
                code.push_str("#pagebreak(weak: true)\n");
            }
        }
        code
    }

    fn preamble(
        code: &mut String,
        title: &str,
        page: &PageConfig,
        header: Option<&RenderedBand>,
        footer: Option<&RenderedBand>,
        skip_header_on_first_page: bool,
    ) {
        let header_h = header.map_or(0.0, |b| b.height_mm);
        let footer_h = footer.map_or(0.0, |b| b.height_mm);
        writeln!(code, "#set document(title: {})", lit(title)).unwrap();
        writeln!(code, "#set text(font: {}, size: 10pt, lang: \"pt\", top-edge: \"ascender\", bottom-edge: \"descender\")", font_list("Arial")).unwrap();
        code.push_str("#set par(leading: 0.25em, spacing: 0pt, justify: false)\n");
        code.push_str("#set block(spacing: 0pt)\n");
        code.push_str("#set page(\n");
        writeln!(code, "  width: {}, height: {},", mm(page.width_mm), mm(page.height_mm)).unwrap();
        writeln!(
            code,
            "  margin: (top: {}, bottom: {}, left: {}, right: {}),",
            mm(page.margin_top_mm + header_h),
            mm(page.margin_bottom_mm + footer_h),
            mm(page.margin_left_mm),
            mm(page.margin_right_mm)
        )
        .unwrap();
        code.push_str("  header-ascent: 0pt, footer-descent: 0pt,\n");
        if let Some(h) = header {
            code.push_str(if skip_header_on_first_page { "  header: context if counter(page).get().first() > 1 { " } else { "  header: " });
            Self::render_band(code, h);
            code.push_str(if skip_header_on_first_page { " },\n" } else { ",\n" });
        }
        if let Some(f) = footer {
            code.push_str("  footer: ");
            Self::render_band(code, f);
            code.push_str(",\n");
        }
        code.push_str(")\n");
    }

    /// Emite a banda em modo código (sem `#` inicial): `block(...)[...]`.
    fn render_band(code: &mut String, band: &RenderedBand) {
        writeln!(code, "block(width: 100%, height: {}, breakable: false)[", mm(band.height_mm)).unwrap();
        for el in &band.elements {
            code.push_str("  ");
            Self::render_element(code, el);
            code.push('\n');
        }
        code.push(']');
    }

    fn render_element(code: &mut String, el: &RenderedElement) {
        match el {
            RenderedElement::TextBox {
                x_mm, y_mm, width_mm, height_mm, text, font_family, font_size_pt, bold, italic, underline,
                color_hex, bg_color_hex, border_pt, border_color_hex, padding_mm, align, valign, ..
            } => {
                let mut body = format!(
                    "text(font: {}, size: {}pt, weight: \"{}\", style: \"{}\", fill: {}, {})",
                    font_list(font_family),
                    num(*font_size_pt),
                    if *bold { "bold" } else { "regular" },
                    if *italic { "italic" } else { "normal" },
                    color(color_hex),
                    content(text)
                );
                if *underline {
                    body = format!("underline({body})");
                }
                write!(
                    code,
                    "#place(top + left, dx: {}, dy: {}, block(width: {}, height: {}, inset: {}, clip: true, fill: {}, stroke: {}, align({} + {}, {})))",
                    mm(*x_mm), mm(*y_mm), mm(*width_mm), mm(*height_mm), mm(*padding_mm),
                    bg_color_hex.as_deref().map_or("none".into(), color),
                    stroke(*border_pt, border_color_hex),
                    h_align(align), v_align(valign), body
                )
                .unwrap();
            }
            RenderedElement::Image { x_mm, y_mm, width_mm, height_mm, virtual_path, fit } => {
                write!(
                    code,
                    "#place(top + left, dx: {}, dy: {}, image({}, width: {}, height: {}, fit: {}))",
                    mm(*x_mm), mm(*y_mm), lit(virtual_path), mm(*width_mm), mm(*height_mm), lit(fit)
                )
                .unwrap();
            }
            RenderedElement::Line { x1_mm, y1_mm, x2_mm, y2_mm, stroke_pt, color_hex } => {
                write!(
                    code,
                    "#place(top + left, line(start: ({}, {}), end: ({}, {}), stroke: {}))",
                    mm(*x1_mm), mm(*y1_mm), mm(*x2_mm), mm(*y2_mm), stroke(stroke_pt.max(0.1), color_hex)
                )
                .unwrap();
            }
            RenderedElement::Rect { x_mm, y_mm, width_mm, height_mm, stroke_pt, stroke_color_hex, fill_hex, radius_mm } => {
                write!(
                    code,
                    "#place(top + left, dx: {}, dy: {}, rect(width: {}, height: {}, radius: {}, fill: {}, stroke: {}))",
                    mm(*x_mm), mm(*y_mm), mm(*width_mm), mm(*height_mm), mm(*radius_mm),
                    fill_hex.as_deref().map_or("none".into(), color),
                    stroke(*stroke_pt, stroke_color_hex)
                )
                .unwrap();
            }
            RenderedElement::Table { .. } => {}
        }
    }

    fn render_table(code: &mut String, table: &RenderedElement) {
        let RenderedElement::Table { x_mm, y_mm: _, columns_width_mm, headers, rows, footers, repeat_header, font_family, font_size_pt, zebra_hex } = table else {
            return;
        };
        let cols = columns_width_mm.iter().map(|w| mm(*w)).collect::<Vec<_>>().join(", ");
        let header_rows = headers.len();
        writeln!(code, "#pad(left: {})[#text(font: {}, size: {}pt)[#table(", mm(*x_mm), font_list(font_family), num(*font_size_pt)).unwrap();
        writeln!(code, "  columns: ({cols},), stroke: 0.5pt + rgb(\"#CBD5E1\"), inset: (x: 1.2mm, y: 1mm),").unwrap();
        if let Some(z) = zebra_hex {
            writeln!(code, "  fill: (_, y) => if y >= {header_rows} and calc.odd(y - {header_rows}) {{ {} }} else {{ none }},", color(z)).unwrap();
        }
        if !headers.is_empty() {
            writeln!(code, "  table.header(repeat: {repeat_header},").unwrap();
            for c in headers.iter().flatten() {
                Self::write_cell(code, c);
            }
            code.push_str("  ),\n");
        }
        for c in rows.iter().flatten() {
            Self::write_cell(code, c);
        }
        if !footers.is_empty() {
            code.push_str("  table.footer(repeat: false,\n");
            for c in footers.iter().flatten() {
                Self::write_cell(code, c);
            }
            code.push_str("  ),\n");
        }
        code.push_str(")]]\n");
    }

    fn write_cell(code: &mut String, cell: &TableCell) {
        writeln!(
            code,
            "    table.cell(colspan: {}, rowspan: {}, align: {}, fill: {}, text(weight: \"{}\", fill: {}, {})),",
            cell.colspan.max(1),
            cell.rowspan.max(1),
            h_align(&cell.align),
            cell.bg_color_hex.as_deref().map_or("auto".into(), color),
            if cell.bold { "bold" } else { "regular" },
            color(&cell.text_color_hex),
            content(&cell.text)
        )
        .unwrap();
    }
}

/// Literal de string Typst com escape completo.
pub(crate) fn lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Conteúdo de texto: literal simples, ou bloco com contadores de página quando há marcadores.
fn content(text: &str) -> String {
    if !text.contains('\u{E000}') {
        return lit(text);
    }
    let mut parts = Vec::new();
    let mut rest = text;
    loop {
        let next = [(PAGE_NUMBER_MARK, "context counter(page).display()"), (TOTAL_PAGES_MARK, "context counter(page).final().first()")]
            .into_iter()
            .filter_map(|(m, code)| rest.find(m).map(|p| (p, m, code)))
            .min_by_key(|x| x.0);
        match next {
            Some((pos, mark, code)) => {
                if pos > 0 {
                    parts.push(format!("#{}", lit(&rest[..pos])));
                }
                parts.push(format!("#{code}"));
                rest = &rest[pos + mark.len()..];
            }
            None => {
                if !rest.is_empty() {
                    parts.push(format!("#{}", lit(rest)));
                }
                break;
            }
        }
    }
    format!("[{}]", parts.join(""))
}

fn font_list(primary: &str) -> String {
    let mut fonts = vec![primary];
    fonts.extend(FALLBACK_FONTS.iter().filter(|f| !f.eq_ignore_ascii_case(primary)));
    format!("({},)", fonts.iter().map(|f| lit(f)).collect::<Vec<_>>().join(", "))
}

fn num(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn mm(v: f64) -> String {
    format!("{}mm", num(v))
}

fn color(hex: &str) -> String {
    let h = hex.trim().trim_start_matches('#');
    let valid = matches!(h.len(), 3 | 4 | 6 | 8) && h.chars().all(|c| c.is_ascii_hexdigit());
    format!("rgb(\"#{}\")", if valid { h } else { "000000" })
}

fn stroke(width_pt: f64, hex: &str) -> String {
    if width_pt <= 0.0 { "none".into() } else { format!("{}pt + {}", num(width_pt), color(hex)) }
}

fn h_align(a: &str) -> &'static str {
    match a {
        "center" => "center",
        "right" => "right",
        // justify é aplicado via par; alinhamento horizontal fica à esquerda
        _ => "left",
    }
}

fn v_align(a: &str) -> &'static str {
    match a {
        "middle" => "horizon",
        "bottom" => "bottom",
        _ => "top",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_user_text_as_string_literal() {
        assert_eq!(lit("a\"b\\c #x *y* $z$"), r#""a\"b\\c #x *y* $z$""#);
    }

    #[test]
    fn page_markers_become_counters() {
        let t = format!("Página {PAGE_NUMBER_MARK} de {TOTAL_PAGES_MARK}");
        assert_eq!(
            content(&t),
            "[#\"Página \"#context counter(page).display()#\" de \"#context counter(page).final().first()]"
        );
    }

    #[test]
    fn invalid_colors_fall_back_to_black() {
        assert_eq!(color("#12"), "rgb(\"#000000\")");
        assert_eq!(color("ff0000"), "rgb(\"#ff0000\")");
    }
}

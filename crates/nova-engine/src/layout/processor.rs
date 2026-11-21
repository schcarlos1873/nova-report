use std::path::Path;

use nova_domain::report::{
    Align, Band, BandType, ElementKind, ImageFit, ImageKind, LineDirection, ReportDefinition, ReportElement,
    TableSpec, VerticalAlign, image_ref,
};
use polars::prelude::DataFrame;

use super::assets::{AssetResolver, field_image_source};
use crate::data::DataContext;
use crate::expression::{FormulaEngine, PAGE_NUMBER_MARK, TOTAL_PAGES_MARK};
use crate::render::{PageConfig, RenderedBand, RenderedElement, RenderedReport, TableCell};
use crate::EngineResult;

const PT_TO_MM: f64 = 25.4 / 72.0;

/// Segmento contínuo de linhas com a mesma chave de grupo.
#[derive(Debug, Clone, Copy)]
struct Segment {
    start: usize,
    len: usize,
}

pub struct ReportProcessor<'a> {
    def: &'a ReportDefinition,
    data: &'a DataContext,
    engine: FormulaEngine,
    assets: AssetResolver,
    warnings: Vec<String>,
    main: Option<DataFrame>,
}

impl<'a> ReportProcessor<'a> {
    pub fn new(def: &'a ReportDefinition, data: &'a DataContext, base_dir: Option<&Path>) -> Self {
        let mut engine = FormulaEngine::new(&data.parameters, &def.meta.title);
        engine.set_param_labels(&data.param_labels);
        engine.set_report_info("Author", def.meta.author.clone().unwrap_or_default());
        engine.set_report_info("Description", def.meta.description.clone().unwrap_or_default());
        if let Some(p) = &def.source_path {
            engine.set_report_info("FileName", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            engine.set_report_info("FilePath", p.to_string_lossy());
        }
        let main = def.main_query_name().and_then(|q| data.datasets.get(&q).cloned());
        Self { def, data, engine, assets: AssetResolver::new(base_dir), warnings: vec![], main }
    }

    pub fn process(mut self) -> EngineResult<RenderedReport> {
        let geo = self.def.page_geometry();
        self.engine.set_dataset(self.main.clone());
        let height = self.engine.dataset_height();
        let first = (height > 0).then_some(0);
        let last = height.checked_sub(1);

        let mut flow = Vec::new();

        // Page header/footer: contexto do relatório inteiro, primeira linha.
        self.engine.set_full_range();
        self.engine.set_row(first);
        let page_header = self.band_of(BandType::PageHeader).and_then(|b| self.render_band(b));
        let page_footer = self.band_of(BandType::PageFooter).and_then(|b| self.render_band(b));

        if let Some(b) = self.band_of(BandType::ReportHeader) {
            flow.extend(self.render_band(b));
        }

        if let Some(detail) = self.band_of(BandType::Detail) {
            self.process_detail(detail, height, &mut flow)?;
        }

        self.engine.set_full_range();
        self.engine.set_row(last);
        if let Some(b) = self.band_of(BandType::ReportFooter) {
            flow.extend(self.render_band(b));
        }

        Ok(RenderedReport {
            title: self.def.meta.title.clone(),
            page: PageConfig {
                width_mm: geo.width_mm,
                height_mm: geo.height_mm,
                margin_top_mm: geo.margins.top,
                margin_bottom_mm: geo.margins.bottom,
                margin_left_mm: geo.margins.left,
                margin_right_mm: geo.margins.right,
            },
            page_header,
            page_footer,
            flow_bands: flow,
            assets: std::mem::take(&mut self.assets.files),
            warnings: self.warnings,
        })
    }

    fn band_of(&self, t: BandType) -> Option<&'a Band> {
        self.def.band(t)
    }

    fn group_band(&self, t: BandType, group: &str) -> Option<&'a Band> {
        self.def.bands.iter().find(|b| b.band_type == t && b.group.as_deref() == Some(group))
    }

    fn process_detail(&mut self, detail: &'a Band, height: usize, flow: &mut Vec<RenderedBand>) -> EngineResult<()> {
        let groups = &self.def.groups;
        // Passo 1: chaves de grupo por linha → segmentos por nível.
        let mut segments: Vec<Vec<Segment>> = vec![Vec::new(); groups.len()];
        if !groups.is_empty() {
            let mut prev: Vec<Option<String>> = vec![None; groups.len()];
            for row in 0..height {
                self.engine.set_row(Some(row));
                let mut broke = false;
                for (lvl, g) in groups.iter().enumerate() {
                    let key = self.engine.eval_key(&g.expression)?;
                    if broke || prev[lvl].as_ref() != Some(&key) {
                        broke = true;
                        segments[lvl].push(Segment { start: row, len: 0 });
                        prev[lvl] = Some(key);
                    }
                    segments[lvl].last_mut().expect("segmento aberto").len += 1;
                }
            }
        }

        // Passo 2: emissão das bandas.
        let mut cursor = vec![0usize; groups.len()];
        for row in 0..height {
            for (lvl, g) in groups.iter().enumerate() {
                let seg = segments[lvl][cursor[lvl]];
                if seg.start == row {
                    self.engine.set_range(seg.start, seg.len);
                    self.engine.set_row(Some(row));
                    if let Some(b) = self.group_band(BandType::GroupHeader, &g.name) {
                        if let Some(mut rb) = self.render_band(b) {
                            rb.page_break_before = g.page_break_between && row > 0;
                            flow.push(rb);
                        }
                    } else if g.page_break_between && row > 0 {
                        if let Some(last) = flow.last_mut() {
                            last.page_break_after = true;
                        }
                    }
                }
            }

            match groups.len() {
                0 => self.engine.set_full_range(),
                n => {
                    let seg = segments[n - 1][cursor[n - 1]];
                    self.engine.set_range(seg.start, seg.len);
                }
            }
            self.engine.set_row(Some(row));
            flow.extend(self.render_band(detail));

            for (lvl, g) in groups.iter().enumerate().rev() {
                let seg = segments[lvl][cursor[lvl]];
                if seg.start + seg.len - 1 == row {
                    self.engine.set_range(seg.start, seg.len);
                    self.engine.set_row(Some(row));
                    if let Some(b) = self.group_band(BandType::GroupFooter, &g.name) {
                        flow.extend(self.render_band(b));
                    }
                    cursor[lvl] += 1;
                }
            }
        }
        Ok(())
    }

    /// Avalia uma banda no contexto corrente. Retorna `None` se invisível.
    fn render_band(&mut self, band: &Band) -> Option<RenderedBand> {
        if let Some(v) = &band.visible {
            match self.engine.eval_bool(v) {
                Ok(false) => return None,
                Ok(true) => {}
                Err(e) => self.warn(&band.id, e),
            }
        }
        let mut height = band.height;
        let mut elements = Vec::with_capacity(band.elements.len());
        for el in &band.elements {
            if let Some(v) = &el.visible {
                match self.engine.eval_bool(v) {
                    Ok(false) => continue,
                    Ok(true) => {}
                    Err(e) => {
                        self.warn(&el.id, e);
                        continue;
                    }
                }
            }
            if let Some((rendered, needed_bottom)) = self.render_element(el) {
                height = height.max(needed_bottom);
                elements.push(rendered);
            }
        }
        Some(RenderedBand {
            source_band_id: band.id.clone(),
            band_type: serde_json::to_value(band.band_type).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
            height_mm: height,
            keep_together: band.keep_together,
            page_break_before: false,
            page_break_after: band.page_break_after,
            elements,
        })
    }

    /// Origem concreta de uma imagem: `image:<nome>` é resolvido em `images` (o tipo `database` lê o
    /// campo no registro corrente); os demais valores passam direto.
    fn image_source(&mut self, source: &str) -> Result<String, String> {
        let Some(name) = image_ref(source) else { return Ok(source.to_string()) };
        let img = self.def.find_image(name).ok_or_else(|| format!("imagem '{name}' não existe"))?;
        match img.kind {
            ImageKind::File | ImageKind::Embedded => Ok(img.value.clone()),
            ImageKind::Url => Err(format!("a imagem '{name}' (URL) não foi baixada antes da geração")),
            ImageKind::Database => {
                let field = img.field.as_deref().unwrap_or("").replace('"', "");
                let value = self.engine.eval_string(&format!("Fields[\"{field}\"]")).map_err(|e| e.to_string())?;
                field_image_source(&value)
            }
        }
    }

    fn warn(&mut self, id: &str, e: impl std::fmt::Display) {
        let msg = format!("{id}: {e}");
        if !self.warnings.contains(&msg) && self.warnings.len() < 200 {
            self.warnings.push(msg);
        }
    }

    /// Retorna o elemento renderizado e a coordenada inferior necessária (para can-grow).
    fn render_element(&mut self, el: &ReportElement) -> Option<(RenderedElement, f64)> {
        let s = &el.style;
        let bottom = el.y + el.height;
        let eval_color = |engine: &mut FormulaEngine, expr: &Option<String>| {
            expr.as_ref().and_then(|e| engine.eval_string(e).ok()).filter(|c| !c.is_empty())
        };
        match &el.kind {
            ElementKind::Label { .. } | ElementKind::Expression { .. } => {
                let text = match &el.kind {
                    ElementKind::Label { text } => {
                        text.replace("{PAGE}", PAGE_NUMBER_MARK).replace("{PAGES}", TOTAL_PAGES_MARK)
                    }
                    ElementKind::Expression { expression, format } => {
                        match self.engine.eval_formatted(expression, format.as_deref()) {
                            Ok(t) => t,
                            Err(e) => {
                                self.warn(&el.id, e);
                                "#ERRO".into()
                            }
                        }
                    }
                    _ => unreachable!(),
                };
                let color = eval_color(&mut self.engine, &el.color_expression).unwrap_or_else(|| s.color.clone());
                let bg = eval_color(&mut self.engine, &el.background_expression).or_else(|| s.background.clone());
                let mut h = el.height;
                if el.can_grow {
                    h = h.max(estimate_text_height(&text, el.width - 2.0 * s.padding, s.font_size) + 2.0 * s.padding);
                }
                Some((
                    RenderedElement::TextBox {
                        x_mm: el.x,
                        y_mm: el.y,
                        width_mm: el.width,
                        height_mm: h,
                        text,
                        font_family: s.font_family.clone(),
                        font_size_pt: s.font_size,
                        bold: s.bold,
                        italic: s.italic,
                        underline: s.underline,
                        color_hex: color,
                        bg_color_hex: bg,
                        border_pt: s.border_width,
                        border_color_hex: s.border_color.clone(),
                        padding_mm: s.padding,
                        align: align_str(s.align).into(),
                        valign: match s.vertical_align {
                            VerticalAlign::Top => "top",
                            VerticalAlign::Middle => "middle",
                            VerticalAlign::Bottom => "bottom",
                        }
                        .into(),
                        can_grow: el.can_grow,
                    },
                    el.y + h,
                ))
            }
            ElementKind::Image { source, fit } => match self.image_source(source).and_then(|s| self.assets.resolve(&s)) {
                Ok(vpath) => Some((
                    RenderedElement::Image {
                        x_mm: el.x,
                        y_mm: el.y,
                        width_mm: el.width,
                        height_mm: el.height,
                        virtual_path: vpath,
                        fit: match fit {
                            ImageFit::Contain => "contain",
                            ImageFit::Cover => "cover",
                            ImageFit::Stretch => "stretch",
                        }
                        .into(),
                    },
                    bottom,
                )),
                Err(e) => {
                    self.warn(&el.id, format!("imagem: {e}"));
                    None
                }
            },
            ElementKind::Line { direction } => {
                let (x2, y2) = match direction {
                    LineDirection::Horizontal => (el.x + el.width, el.y),
                    LineDirection::Vertical => (el.x, el.y + el.height),
                    LineDirection::Diagonal => (el.x + el.width, el.y + el.height),
                };
                Some((
                    RenderedElement::Line {
                        x1_mm: el.x,
                        y1_mm: el.y,
                        x2_mm: x2,
                        y2_mm: y2,
                        stroke_pt: if s.border_width > 0.0 { s.border_width } else { 0.5 },
                        color_hex: s.border_color.clone(),
                    },
                    bottom,
                ))
            }
            ElementKind::Rectangle { radius } => {
                let fill = eval_color(&mut self.engine, &el.background_expression).or_else(|| s.background.clone());
                Some((
                    RenderedElement::Rect {
                        x_mm: el.x,
                        y_mm: el.y,
                        width_mm: el.width,
                        height_mm: el.height,
                        stroke_pt: s.border_width,
                        stroke_color_hex: s.border_color.clone(),
                        fill_hex: fill,
                        radius_mm: *radius,
                    },
                    bottom,
                ))
            }
            ElementKind::Table(spec) => self.render_table(el, spec).map(|t| (t, bottom)),
        }
    }

    /// Tabelas usam seu próprio dataset; o contexto do relatório é restaurado ao final.
    fn render_table(&mut self, el: &ReportElement, spec: &TableSpec) -> Option<RenderedElement> {
        let Some(df) = self.data.datasets.get(&spec.data_source).cloned() else {
            self.warn(&el.id, format!("dataset '{}' não carregado", spec.data_source));
            return None;
        };
        let s = &el.style;
        let cell = |text: String, align: Align, bold: bool, bg: Option<String>| TableCell {
            text,
            colspan: 1,
            rowspan: 1,
            align: align_str(align).into(),
            bold,
            bg_color_hex: bg,
            text_color_hex: s.color.clone(),
        };
        let headers = vec![
            spec.columns
                .iter()
                .map(|c| cell(c.header.clone(), c.align, true, Some(spec.header_background.clone())))
                .collect(),
        ];

        let ((start, len), row) = self.engine.position();
        self.engine.set_dataset(Some(df.clone()));
        let mut rows = Vec::with_capacity(df.height());
        for r in 0..df.height() {
            self.engine.set_row(Some(r));
            let mut row = Vec::with_capacity(spec.columns.len());
            for c in &spec.columns {
                let text = match self.engine.eval_formatted(&c.expression, c.format.as_deref()) {
                    Ok(t) => t,
                    Err(e) => {
                        self.warn(&el.id, e);
                        "#ERRO".into()
                    }
                };
                row.push(cell(text, c.align, false, None));
            }
            rows.push(row);
        }
        let mut footers = Vec::new();
        if spec.columns.iter().any(|c| c.footer.is_some()) {
            self.engine.set_row(df.height().checked_sub(1));
            let mut row = Vec::new();
            for c in &spec.columns {
                let text = match &c.footer {
                    Some(f) => self.engine.eval_formatted(f, c.format.as_deref()).unwrap_or_else(|e| {
                        self.warn(&el.id, e);
                        "#ERRO".into()
                    }),
                    None => String::new(),
                };
                row.push(cell(text, c.align, true, Some(spec.header_background.clone())));
            }
            footers.push(row);
        }
        // restaura o contexto do dataset principal
        self.engine.set_dataset(self.main.clone());
        self.engine.set_range(start, len);
        self.engine.set_row(row);

        Some(RenderedElement::Table {
            x_mm: el.x,
            y_mm: el.y,
            columns_width_mm: spec.columns.iter().map(|c| c.width).collect(),
            headers,
            rows,
            footers,
            repeat_header: spec.repeat_header,
            font_family: s.font_family.clone(),
            font_size_pt: s.font_size,
            zebra_hex: spec.zebra_background.clone(),
        })
    }
}

fn align_str(a: Align) -> &'static str {
    match a {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
        Align::Justify => "justify",
    }
}

/// Estimativa conservadora de altura para texto com quebra automática (can-grow).
fn estimate_text_height(text: &str, width_mm: f64, font_pt: f64) -> f64 {
    let char_w = font_pt * PT_TO_MM * 0.5;
    let per_line = (width_mm / char_w).floor().max(1.0) as usize;
    let lines: usize = text.split('\n').map(|l| l.chars().count().div_ceil(per_line).max(1)).sum();
    lines as f64 * font_pt * PT_TO_MM * 1.25
}

#[cfg(test)]
mod tests;

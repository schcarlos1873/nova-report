use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageConfig {
    pub width_mm: f64,
    pub height_mm: f64,
    pub margin_top_mm: f64,
    pub margin_bottom_mm: f64,
    pub margin_left_mm: f64,
    pub margin_right_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum RenderedElement {
    TextBox {
        x_mm: f64,
        y_mm: f64,
        width_mm: f64,
        height_mm: f64,
        text: String,
        font_family: String,
        font_size_pt: f64,
        bold: bool,
        italic: bool,
        underline: bool,
        color_hex: String,
        bg_color_hex: Option<String>,
        border_pt: f64,
        border_color_hex: String,
        padding_mm: f64,
        align: String,
        valign: String,
        can_grow: bool,
    },
    Table {
        x_mm: f64,
        y_mm: f64,
        columns_width_mm: Vec<f64>,
        headers: Vec<Vec<TableCell>>,
        rows: Vec<Vec<TableCell>>,
        footers: Vec<Vec<TableCell>>,
        repeat_header: bool,
        font_family: String,
        font_size_pt: f64,
        zebra_hex: Option<String>,
    },
    Image {
        x_mm: f64,
        y_mm: f64,
        width_mm: f64,
        height_mm: f64,
        virtual_path: String,
        fit: String,
    },
    Line {
        x1_mm: f64,
        y1_mm: f64,
        x2_mm: f64,
        y2_mm: f64,
        stroke_pt: f64,
        color_hex: String,
    },
    Rect {
        x_mm: f64,
        y_mm: f64,
        width_mm: f64,
        height_mm: f64,
        stroke_pt: f64,
        stroke_color_hex: String,
        fill_hex: Option<String>,
        radius_mm: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TableCell {
    pub text: String,
    pub colspan: usize,
    pub rowspan: usize,
    pub align: String,
    pub bold: bool,
    pub bg_color_hex: Option<String>,
    pub text_color_hex: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RenderedBand {
    pub source_band_id: String,
    /// Tipo da banda de origem (`reportHeader`, `detail`...).
    pub band_type: String,
    pub height_mm: f64,
    pub keep_together: bool,
    pub page_break_before: bool,
    pub page_break_after: bool,
    pub elements: Vec<RenderedElement>,
}

/// Resultado do processamento de bandas: tudo já avaliado, pronto para o transpilador.
#[derive(Debug, Clone, Serialize)]
pub struct RenderedReport {
    pub title: String,
    pub page: PageConfig,
    pub page_header: Option<RenderedBand>,
    pub page_footer: Option<RenderedBand>,
    pub flow_bands: Vec<RenderedBand>,
    /// Arquivos binários referenciados (imagens), indexados pelo caminho virtual.
    #[serde(skip)]
    pub assets: BTreeMap<String, Vec<u8>>,
    /// Avisos não fatais (expressões com erro, imagens ausentes...).
    pub warnings: Vec<String>,
}

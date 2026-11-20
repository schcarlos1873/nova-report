use serde::{Deserialize, Serialize};

use super::ReportMeta;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PaperSize {
    A3,
    #[default]
    A4,
    A5,
    Letter,
    Legal,
    Custom,
}

impl PaperSize {
    /// Dimensões em retrato (largura, altura) em milímetros.
    pub fn portrait_mm(self) -> Option<(f64, f64)> {
        Some(match self {
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
            PaperSize::Custom => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PaperSizeMm {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Margins {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl Default for Margins {
    fn default() -> Self {
        Self { top: 10.0, bottom: 10.0, left: 15.0, right: 15.0 }
    }
}

/// Value object com a geometria efetiva da página (já considerando orientação).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    pub width_mm: f64,
    pub height_mm: f64,
    pub margins: Margins,
}

impl PageGeometry {
    pub fn from_meta(meta: &ReportMeta) -> Self {
        let (w, h) = meta
            .paper_size
            .portrait_mm()
            .or_else(|| meta.custom_size.map(|c| (c.width, c.height)))
            .unwrap_or((210.0, 297.0));
        let (width_mm, height_mm) = match meta.orientation {
            Orientation::Portrait => (w.min(h), w.max(h)),
            Orientation::Landscape => (w.max(h), w.min(h)),
        };
        Self { width_mm, height_mm, margins: meta.margins }
    }

    pub fn printable_width(&self) -> f64 {
        self.width_mm - self.margins.left - self.margins.right
    }

    pub fn printable_height(&self) -> f64 {
        self.height_mm - self.margins.top - self.margins.bottom
    }
}

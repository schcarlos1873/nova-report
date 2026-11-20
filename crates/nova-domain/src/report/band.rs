use serde::{Deserialize, Serialize};

use super::ReportElement;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BandType {
    ReportHeader,
    PageHeader,
    GroupHeader,
    Detail,
    GroupFooter,
    PageFooter,
    ReportFooter,
}

impl BandType {
    pub const ALL: [BandType; 7] = [
        BandType::ReportHeader,
        BandType::PageHeader,
        BandType::GroupHeader,
        BandType::Detail,
        BandType::GroupFooter,
        BandType::PageFooter,
        BandType::ReportFooter,
    ];

    /// Ordem canônica de exibição/processamento.
    pub fn order(self) -> u8 {
        match self {
            BandType::ReportHeader => 0,
            BandType::PageHeader => 1,
            BandType::GroupHeader => 2,
            BandType::Detail => 3,
            BandType::GroupFooter => 4,
            BandType::PageFooter => 5,
            BandType::ReportFooter => 6,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            BandType::ReportHeader => "report_header",
            BandType::PageHeader => "page_header",
            BandType::GroupHeader => "group_header",
            BandType::Detail => "detail",
            BandType::GroupFooter => "group_footer",
            BandType::PageFooter => "page_footer",
            BandType::ReportFooter => "report_footer",
        }
    }

    /// Bandas que podem existir no máximo uma vez.
    pub fn is_singleton(self) -> bool {
        !matches!(self, BandType::GroupHeader | BandType::GroupFooter)
    }

    pub fn is_group_band(self) -> bool {
        matches!(self, BandType::GroupHeader | BandType::GroupFooter)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Band {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub band_type: BandType,
    /// Altura em milímetros.
    pub height: f64,
    /// Nome do grupo (apenas para groupHeader/groupFooter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Query que alimenta a banda (apenas detail).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep_together: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub page_break_after: bool,
    /// Expressão booleana que controla a visibilidade da banda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<String>,
    #[serde(default)]
    pub elements: Vec<ReportElement>,
}

impl Band {
    pub fn new(band_type: BandType, height: f64) -> Self {
        Self {
            id: String::new(),
            band_type,
            height,
            group: None,
            data_source: None,
            keep_together: false,
            page_break_after: false,
            visible: None,
            elements: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDef {
    pub name: String,
    /// Expressão que define a chave do grupo (ex.: `Fields.cliente`).
    pub expression: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub page_break_between: bool,
}

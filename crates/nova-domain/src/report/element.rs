use serde::{Deserialize, Serialize};

/// Elemento posicionado dentro de uma banda. Coordenadas e dimensões em milímetros,
/// relativas ao canto superior esquerdo da banda (dentro das margens da página).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportElement {
    pub id: String,
    #[serde(flatten)]
    pub kind: ElementKind,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub style: Style,
    /// Expressão booleana de visibilidade (ex.: `Fields.valor > 0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<String>,
    /// Expressão que retorna a cor do texto (ex.: `IIF(Fields.valor < 0, "#DC2626", "#000000")`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_expression: Option<String>,
    /// Expressão que retorna a cor de fundo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_expression: Option<String>,
    /// Permite que a banda cresça verticalmente para acomodar o texto.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub can_grow: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ElementKind {
    /// Texto estático. Suporta `{PAGE}` e `{PAGES}` como marcadores.
    Label { text: String },
    /// Expressão Rhai avaliada por linha/grupo.
    #[serde(rename_all = "camelCase")]
    Expression {
        expression: String,
        /// Formato opcional aplicado ao resultado: `currency:BRL`, `number:2`, `date:%d/%m/%Y`, `percent:1`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        format: Option<String>,
    },
    /// Imagem: caminho relativo ao `.nrep`, absoluto ou `data:` URI.
    Image {
        source: String,
        #[serde(default)]
        fit: ImageFit,
    },
    Line {
        #[serde(default)]
        direction: LineDirection,
    },
    Rectangle {
        #[serde(default)]
        radius: f64,
    },
    /// Tabela com quebra de página dinâmica e cabeçalho repetido.
    Table(TableSpec),
}

impl ElementKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            ElementKind::Label { .. } => "label",
            ElementKind::Expression { .. } => "expression",
            ElementKind::Image { .. } => "image",
            ElementKind::Line { .. } => "line",
            ElementKind::Rectangle { .. } => "rectangle",
            ElementKind::Table(_) => "table",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageFit {
    #[default]
    Contain,
    Cover,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LineDirection {
    #[default]
    Horizontal,
    Vertical,
    Diagonal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSpec {
    pub data_source: String,
    pub columns: Vec<TableColumn>,
    #[serde(default = "yes")]
    pub repeat_header: bool,
    #[serde(default = "default_header_bg")]
    pub header_background: String,
    /// Cor das linhas alternadas (zebra), se definida.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zebra_background: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableColumn {
    pub header: String,
    pub expression: String,
    /// Largura em milímetros.
    pub width: f64,
    #[serde(default)]
    pub align: Align,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Expressão de rodapé (ex.: `SUM("valor_total")`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
}

fn yes() -> bool {
    true
}
fn default_header_bg() -> String {
    "#E2E8F0".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Style {
    pub font_family: String,
    /// Tamanho em pontos tipográficos.
    pub font_size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    pub align: Align,
    pub vertical_align: VerticalAlign,
    /// Espessura da borda / traço em pontos (0 = sem borda).
    pub border_width: f64,
    pub border_color: String,
    /// Espaçamento interno em milímetros.
    pub padding: f64,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            font_family: "Arial".into(),
            font_size: 10.0,
            bold: false,
            italic: false,
            underline: false,
            color: "#000000".into(),
            background: None,
            align: Align::Left,
            vertical_align: VerticalAlign::Top,
            border_width: 0.0,
            border_color: "#000000".into(),
            padding: 0.0,
        }
    }
}

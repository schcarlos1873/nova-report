//! Agregado `ReportDefinition` — a representação tipada do arquivo `.nrep`.
//!
//! O formato é declarativo e determinístico: a mesma definição sempre serializa para
//! o mesmo texto (ordem de campos fixa e bandas em ordem canônica).

mod band;
mod data_source;
mod element;
mod geometry;
mod image;
pub mod sql;
mod validation;

pub use band::{Band, BandType, GroupDef};
pub use data_source::{DataSourceDef, ProcedureParam, ProviderKind, QueryDef, QueryKind};
pub use element::{
    Align, ElementKind, ImageFit, LineDirection, ReportElement, Style, TableColumn, TableSpec,
    VerticalAlign,
};
pub use image::{IMAGE_REF_PREFIX, ImageKind, ImageResource, image_ref};
pub use geometry::{Margins, Orientation, PageGeometry, PaperSize, PaperSizeMm};
pub use validation::{Severity, ValidationIssue};

use serde::{Deserialize, Serialize};

use crate::params::ParamType;
use crate::{DomainError, DomainResult};

pub const SCHEMA_URL: &str = "https://novareport.io/schemas/v1/report.json";
pub const CURRENT_SCHEMA_VERSION: &str = "1.0";
pub const FILE_EXTENSION: &str = "nrep";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDefinition {
    #[serde(rename = "$schema", default = "default_schema")]
    pub schema: String,
    #[serde(default = "default_version")]
    pub schema_version: String,
    pub meta: ReportMeta,
    #[serde(default)]
    pub data_sources: Vec<DataSourceDef>,
    #[serde(default)]
    pub parameters: Vec<ParameterDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<GroupDef>,
    #[serde(default)]
    pub bands: Vec<Band>,
    /// Caminho do `.nrep` de origem (não serializado): alimenta `Report.FileName`/`Report.FilePath`.
    #[serde(skip)]
    pub source_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportMeta {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default)]
    pub paper_size: PaperSize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_size: Option<PaperSizeMm>,
    #[serde(default)]
    pub orientation: Orientation,
    #[serde(default)]
    pub margins: Margins,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterDef {
    pub name: String,
    #[serde(rename = "type", default)]
    pub param_type: ParamType,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    /// Aceita vários valores: `:nome` na SQL vira uma lista (`IN (:nome)`) e `Params.nome` um array.
    #[serde(default, skip_serializing_if = "is_false")]
    pub multi_value: bool,
    /// Parâmetros dos quais a lista de valores depende (filtro em cascata). Quando um deles muda,
    /// a lista é recarregada e o valor escolhido é limpo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Lista de valores permitidos vinda de uma query do relatório.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_values: Option<AvailableValues>,
}

/// Origem da lista de valores de um parâmetro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableValues {
    /// Nome de uma query declarada em `dataSources`.
    pub query: String,
    /// Coluna com o valor enviado ao relatório.
    pub value_field: String,
    /// Coluna exibida ao usuário (padrão: o próprio valor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_field: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn default_schema() -> String {
    SCHEMA_URL.to_string()
}
fn default_version() -> String {
    CURRENT_SCHEMA_VERSION.to_string()
}

impl ReportDefinition {
    /// Novo relatório em branco com as bandas mais comuns.
    pub fn new_blank(title: impl Into<String>) -> Self {
        let mut def = Self {
            schema: default_schema(),
            schema_version: default_version(),
            meta: ReportMeta {
                title: title.into(),
                description: None,
                author: None,
                paper_size: PaperSize::A4,
                custom_size: None,
                orientation: Orientation::Portrait,
                margins: Margins::default(),
            },
            data_sources: vec![],
            parameters: vec![],
            images: vec![],
            groups: vec![],
            bands: vec![
                Band::new(BandType::ReportHeader, 20.0),
                Band::new(BandType::PageHeader, 15.0),
                Band::new(BandType::Detail, 8.0),
                Band::new(BandType::PageFooter, 10.0),
                Band::new(BandType::ReportFooter, 15.0),
            ],
            source_path: None,
        };
        def.normalize();
        def
    }

    /// Lê um `.nrep` (JSON), verifica a versão do schema e normaliza.
    pub fn from_json(text: &str) -> DomainResult<Self> {
        let mut def: Self =
            serde_json::from_str(text).map_err(|e| DomainError::Serialization(e.to_string()))?;
        if !def.schema_version.starts_with("1.") {
            return Err(DomainError::UnsupportedSchemaVersion(def.schema_version));
        }
        def.normalize();
        Ok(def)
    }

    /// Serialização determinística (ordem de campos fixa, bandas em ordem canônica).
    pub fn to_canonical_json(&self) -> DomainResult<String> {
        let mut copy = self.clone();
        copy.normalize();
        serde_json::to_string_pretty(&copy).map_err(|e| DomainError::Serialization(e.to_string()))
    }

    /// Garante ids de banda e ordena as bandas na ordem de processamento.
    pub fn normalize(&mut self) {
        let group_pos = |g: &Option<String>| {
            g.as_ref()
                .and_then(|name| self.groups.iter().position(|x| &x.name == name))
                .unwrap_or(usize::MAX)
        };
        let mut keyed: Vec<_> = self
            .bands
            .drain(..)
            .enumerate()
            .map(|(i, b)| {
                let gp = group_pos(&b.group);
                // headers externos→internos, footers internos→externos
                let sub = match b.band_type {
                    BandType::GroupHeader => gp,
                    BandType::GroupFooter => usize::MAX - gp,
                    _ => 0,
                };
                ((b.band_type.order(), sub, i), b)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        self.bands = keyed.into_iter().map(|(_, b)| b).collect();

        let mut used: std::collections::HashSet<String> =
            self.bands.iter().filter(|b| !b.id.is_empty()).map(|b| b.id.clone()).collect();
        for band in &mut self.bands {
            if band.id.is_empty() {
                let base = band.band_type.slug();
                let mut n = 1;
                while used.contains(&format!("{base}_{n}")) {
                    n += 1;
                }
                band.id = format!("{base}_{n}");
                used.insert(band.id.clone());
            }
        }
    }

    pub fn page_geometry(&self) -> PageGeometry {
        PageGeometry::from_meta(&self.meta)
    }

    pub fn band(&self, band_type: BandType) -> Option<&Band> {
        self.bands.iter().find(|b| b.band_type == band_type)
    }

    pub fn find_query(&self, name: &str) -> Option<(&DataSourceDef, &QueryDef)> {
        self.data_sources
            .iter()
            .find_map(|ds| ds.queries.iter().find(|q| q.name == name).map(|q| (ds, q)))
    }

    pub fn find_image(&self, name: &str) -> Option<&ImageResource> {
        self.images.iter().find(|i| i.name == name)
    }

    pub fn all_queries(&self) -> impl Iterator<Item = (&DataSourceDef, &QueryDef)> {
        self.data_sources.iter().flat_map(|ds| ds.queries.iter().map(move |q| (ds, q)))
    }

    /// Query usada só para listar valores de parâmetros (nenhuma banda ou tabela a consome).
    /// Não é executada na geração do relatório.
    pub fn is_parameter_only_query(&self, name: &str) -> bool {
        let lookup = self.parameters.iter().any(|p| p.available_values.as_ref().is_some_and(|a| a.query == name));
        let in_layout = self.bands.iter().any(|b| {
            b.data_source.as_deref() == Some(name)
                || b.elements.iter().any(|e| matches!(&e.kind, ElementKind::Table(t) if t.data_source == name))
        });
        lookup && !in_layout
    }

    /// Dataset principal: o da banda de detalhe, ou a única query de dados existente.
    pub fn main_query_name(&self) -> Option<String> {
        self.band(BandType::Detail)
            .and_then(|b| b.data_source.clone())
            .or_else(|| {
                let mut it = self.all_queries().filter(|(_, q)| !self.is_parameter_only_query(&q.name));
                match (it.next(), it.next()) {
                    (Some((_, q)), None) => Some(q.name.clone()),
                    _ => None,
                }
            })
    }

    /// Executa todas as regras de negócio e devolve os problemas encontrados.
    pub fn validate(&self) -> Vec<ValidationIssue> {
        validation::validate(self)
    }

    /// Falha se houver qualquer problema com severidade `Error`.
    pub fn ensure_valid(&self) -> DomainResult<()> {
        let errors: Vec<_> =
            self.validate().into_iter().filter(|i| i.severity == Severity::Error).collect();
        if errors.is_empty() { Ok(()) } else { Err(DomainError::Validation(errors)) }
    }
}

#[cfg(test)]
mod tests;

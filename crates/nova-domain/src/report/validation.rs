//! Invariantes do agregado `ReportDefinition`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{BandType, ElementKind, ImageKind, ReportDefinition, image_ref, sql};
use crate::params::ParamType;

const TOLERANCE_MM: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub severity: Severity,
    /// Caminho lógico do problema (ex.: `bands[detail_1].elements[txt_1]`).
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}: {}", self.severity, self.path, self.message)
    }
}

struct Issues(Vec<ValidationIssue>);

impl Issues {
    fn error(&mut self, path: impl Into<String>, msg: impl Into<String>) {
        self.0.push(ValidationIssue { severity: Severity::Error, path: path.into(), message: msg.into() });
    }
    fn warn(&mut self, path: impl Into<String>, msg: impl Into<String>) {
        self.0.push(ValidationIssue { severity: Severity::Warning, path: path.into(), message: msg.into() });
    }
}

pub(super) fn validate(def: &ReportDefinition) -> Vec<ValidationIssue> {
    let mut issues = Issues(Vec::new());
    let geo = def.page_geometry();

    if def.meta.title.trim().is_empty() {
        issues.warn("meta.title", "o relatório não possui título");
    }
    if geo.printable_width() <= 0.0 || geo.printable_height() <= 0.0 {
        issues.error("meta.margins", "as margens não deixam área útil na página");
    }

    // ---- fontes de dados / queries
    let mut query_names = HashSet::new();
    let declared_params: HashSet<&str> = def.parameters.iter().map(|p| p.name.as_str()).collect();
    let mut ds_ids = HashSet::new();
    for ds in &def.data_sources {
        if !ds_ids.insert(ds.id.as_str()) {
            issues.error(format!("dataSources[{}]", ds.id), "id de fonte de dados duplicado");
        }
        if ds.connection_ref.is_none()
            && ds.connection_string_ref.is_none()
            && !ds.options.contains_key("path")
            && !ds.options.contains_key("url")
        {
            issues.warn(
                format!("dataSources[{}]", ds.id),
                "fonte de dados sem connectionRef, connectionStringRef ou options.path/url",
            );
        }
        for q in &ds.queries {
            if !query_names.insert(q.name.as_str()) {
                issues.error(format!("dataSources[{}].queries[{}]", ds.id, q.name), "nome de query duplicado");
            }
            let qpath = format!("dataSources[{}].queries[{}]", ds.id, q.name);
            if q.is_procedure() {
                if q.procedure.as_deref().unwrap_or("").trim().is_empty() {
                    issues.error(&qpath, "informe o nome da stored procedure");
                } else if q.effective_sql(ds.provider).is_none() {
                    issues.error(&qpath, format!("o provider {} não suporta stored procedures", ds.provider.id()));
                }
                for p in &q.procedure_params {
                    if p.name.trim().is_empty() {
                        issues.error(&qpath, "parâmetro da procedure sem nome");
                    }
                }
            } else if q.sql.trim().is_empty() {
                issues.error(&qpath, "query vazia");
            }
            if ds.provider.is_sql() {
                for p in sql::named_parameters(&q.parameter_text()) {
                    if !declared_params.contains(p.as_str()) {
                        issues.error(
                            format!("dataSources[{}].queries[{}]", ds.id, q.name),
                            format!("parâmetro ':{p}' usado na query mas não declarado"),
                        );
                    }
                }
            }
        }
    }

    // ---- parâmetros
    let mut seen_params = HashSet::new();
    for p in &def.parameters {
        if !seen_params.insert(p.name.as_str()) {
            issues.error(format!("parameters[{}]", p.name), "parâmetro duplicado");
        }
        if !p.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || p.name.is_empty() {
            issues.error(format!("parameters[{}]", p.name), "nome deve conter apenas letras, números e _");
        }
        let path = format!("parameters[{}]", p.name);
        if p.multi_value && p.param_type == ParamType::Bool {
            issues.error(&path, "parâmetro booleano não pode ter vários valores");
        }
        for d in &p.depends_on {
            if d == &p.name {
                issues.error(&path, "um parâmetro não pode depender de si mesmo");
            } else if !declared_params.contains(d.as_str()) {
                issues.error(&path, format!("dependsOn: parâmetro '{d}' não declarado"));
            }
        }
        if let Some(av) = &p.available_values {
            match def.find_query(&av.query) {
                None => issues.error(&path, format!("availableValues: query '{}' não existe", av.query)),
                Some((ds, q)) => {
                    if av.value_field.trim().is_empty() {
                        issues.error(&path, "availableValues: informe o campo de valor");
                    }
                    if ds.provider.is_sql() {
                        // a lista só é recarregada quando um parâmetro de `dependsOn` muda
                        for used in sql::named_parameters(&q.parameter_text()) {
                            if used == p.name {
                                issues.error(&path, format!("a query de valores '{}' usa o próprio parâmetro", q.name));
                            } else if !p.depends_on.iter().any(|d| d == &used) {
                                issues.warn(&path, format!("a query '{}' usa ':{used}', mas '{used}' não está em dependsOn", q.name));
                            }
                        }
                    }
                }
            }
        } else if !p.depends_on.is_empty() {
            issues.warn(&path, "dependsOn só tem efeito com availableValues");
        }
    }
    if let Some(cycle) = dependency_cycle(&def.parameters) {
        issues.error("parameters", format!("dependência circular entre parâmetros: {}", cycle.join(" → ")));
    }

    // ---- imagens
    let mut image_names = HashSet::new();
    for img in &def.images {
        let path = format!("images[{}]", img.name);
        if img.name.trim().is_empty() {
            issues.error(&path, "imagem sem nome");
        } else if !image_names.insert(img.name.as_str()) {
            issues.error(&path, "nome de imagem duplicado");
        }
        match img.kind {
            ImageKind::Database => match (&img.query, &img.field) {
                (Some(q), _) if !query_names.contains(q.as_str()) => issues.error(&path, format!("query '{q}' não existe")),
                (Some(_), Some(f)) if !f.trim().is_empty() => {}
                _ => issues.error(&path, "informe a query e o campo da imagem"),
            },
            _ if img.value.trim().is_empty() => issues.warn(&path, "imagem sem origem"),
            ImageKind::Embedded if !img.value.starts_with("data:") => issues.error(&path, "imagem embutida deve ser uma data URI"),
            ImageKind::Url if !(img.value.starts_with("http://") || img.value.starts_with("https://")) => {
                issues.error(&path, "URL deve começar com http:// ou https://")
            }
            _ => {}
        }
    }

    // ---- grupos
    let group_names: HashSet<&str> = def.groups.iter().map(|g| g.name.as_str()).collect();
    if group_names.len() != def.groups.len() {
        issues.error("groups", "nomes de grupo duplicados");
    }
    if !def.groups.is_empty() && def.band(BandType::Detail).is_none() {
        issues.error("groups", "grupos exigem uma banda de detalhe");
    }

    // ---- bandas e elementos
    let mut band_ids = HashSet::new();
    let mut element_ids = HashSet::new();
    for bt in BandType::ALL.into_iter().filter(|b| b.is_singleton()) {
        if def.bands.iter().filter(|b| b.band_type == bt).count() > 1 {
            issues.error("bands", format!("a banda {bt:?} só pode existir uma vez"));
        }
    }
    let mut header_space = 0.0;
    for band in &def.bands {
        let bpath = format!("bands[{}]", band.id);
        if !band_ids.insert(band.id.as_str()) {
            issues.error(&bpath, "id de banda duplicado");
        }
        if band.height <= 0.0 {
            issues.error(&bpath, "altura da banda deve ser positiva");
        }
        if matches!(band.band_type, BandType::PageHeader | BandType::PageFooter) {
            header_space += band.height;
        }
        if band.band_type.is_group_band() {
            match &band.group {
                Some(g) if group_names.contains(g.as_str()) => {}
                Some(g) => issues.error(&bpath, format!("grupo '{g}' não existe")),
                None => issues.error(&bpath, "banda de grupo sem grupo associado"),
            }
        }
        if let Some(q) = &band.data_source {
            if !query_names.contains(q.as_str()) {
                issues.error(&bpath, format!("query '{q}' não existe"));
            }
        } else if band.band_type == BandType::Detail && query_names.len() > 1 {
            issues.error(&bpath, "banda de detalhe precisa de dataSource quando há várias queries");
        }

        for el in &band.elements {
            let epath = format!("{bpath}.elements[{}]", el.id);
            if el.id.trim().is_empty() {
                issues.error(&epath, "elemento sem id");
            } else if !element_ids.insert(el.id.as_str()) {
                issues.error(&epath, "id de elemento duplicado");
            }
            if el.width < 0.0 || el.height < 0.0 {
                issues.error(&epath, "dimensões negativas");
            }
            if el.x < -TOLERANCE_MM || el.y < -TOLERANCE_MM {
                issues.warn(&epath, "elemento posicionado fora da banda (coordenada negativa)");
            }
            if el.x + el.width > geo.printable_width() + TOLERANCE_MM {
                issues.warn(&epath, "elemento ultrapassa a largura útil da página");
            }
            if el.y + el.height > band.height + TOLERANCE_MM && !matches!(el.kind, ElementKind::Table(_)) {
                issues.warn(&epath, "elemento ultrapassa a altura da banda");
            }
            match &el.kind {
                ElementKind::Expression { expression, .. } if expression.trim().is_empty() => {
                    issues.error(&epath, "expressão vazia");
                }
                ElementKind::Image { source, .. } if source.trim().is_empty() => {
                    issues.warn(&epath, "imagem sem origem");
                }
                ElementKind::Image { source, .. } => {
                    if let Some(name) = image_ref(source) {
                        if !image_names.contains(name) {
                            issues.error(&epath, format!("imagem '{name}' não existe em images"));
                        }
                    }
                }
                ElementKind::Table(t) => {
                    if !query_names.contains(t.data_source.as_str()) {
                        issues.error(&epath, format!("query '{}' da tabela não existe", t.data_source));
                    }
                    if t.columns.is_empty() {
                        issues.error(&epath, "tabela sem colunas");
                    }
                    let total: f64 = t.columns.iter().map(|c| c.width).sum();
                    if total > geo.printable_width() + TOLERANCE_MM {
                        issues.warn(&epath, "soma das larguras das colunas excede a largura útil");
                    }
                }
                _ => {}
            }
        }
    }
    if header_space >= geo.printable_height() {
        issues.error("bands", "cabeçalho + rodapé de página não deixam espaço para o conteúdo");
    }

    issues.0
}

/// Primeiro ciclo encontrado em `dependsOn` (ex.: `a → b → a`).
fn dependency_cycle(params: &[crate::report::ParameterDef]) -> Option<Vec<String>> {
    use std::collections::HashMap;
    let deps: HashMap<&str, &[String]> = params.iter().map(|p| (p.name.as_str(), p.depends_on.as_slice())).collect();
    // 0 = não visitado, 1 = na pilha, 2 = concluído
    let mut state: HashMap<&str, u8> = HashMap::new();
    fn visit<'a>(n: &'a str, deps: &HashMap<&'a str, &'a [String]>, state: &mut HashMap<&'a str, u8>, stack: &mut Vec<&'a str>) -> Option<Vec<String>> {
        match state.get(n) {
            Some(2) => return None,
            Some(1) => {
                let start = stack.iter().position(|x| *x == n).unwrap_or(0);
                let mut c: Vec<String> = stack[start..].iter().map(|s| s.to_string()).collect();
                c.push(n.to_string());
                return Some(c);
            }
            _ => {}
        }
        state.insert(n, 1);
        stack.push(n);
        for d in deps.get(n).copied().unwrap_or_default() {
            if deps.contains_key(d.as_str()) {
                if let Some(c) = visit(d.as_str(), deps, state, stack) {
                    return Some(c);
                }
            }
        }
        stack.pop();
        state.insert(n, 2);
        None
    }
    let mut stack = Vec::new();
    params.iter().find_map(|p| visit(p.name.as_str(), &deps, &mut state, &mut stack))
}

//! Caso de uso: ciclo de vida de documentos `.nrep`.

use std::path::Path;
use std::sync::Arc;

use nova_domain::ParamValue;
use nova_domain::ports::ReportRepository;
use nova_domain::report::{ReportDefinition, ValidationIssue};
use nova_engine::expression::FormulaEngine;

use crate::AppResult;

#[derive(Clone)]
pub struct DocumentService {
    repository: Arc<dyn ReportRepository>,
}

impl DocumentService {
    pub fn new(repository: Arc<dyn ReportRepository>) -> Self {
        Self { repository }
    }

    pub fn new_report(&self, title: &str) -> ReportDefinition {
        ReportDefinition::new_blank(title)
    }

    pub fn open(&self, path: &Path) -> AppResult<(ReportDefinition, Vec<ValidationIssue>)> {
        let mut def = self.repository.load(path)?;
        def.source_path = Some(path.to_path_buf());
        let issues = self.validate(&def);
        Ok((def, issues))
    }

    /// Salva sempre em JSON canônico. Erros de validação não impedem salvar (rascunhos).
    pub fn save(&self, path: &Path, def: &ReportDefinition) -> AppResult<Vec<ValidationIssue>> {
        self.repository.save(path, def)?;
        Ok(self.validate(def))
    }

    /// Regras do domínio + sintaxe de todas as expressões.
    pub fn validate(&self, def: &ReportDefinition) -> Vec<ValidationIssue> {
        use nova_domain::report::{ElementKind, Severity};
        let mut issues = def.validate();
        let engine = FormulaEngine::new(&std::collections::HashMap::<String, ParamValue>::new(), "");
        let mut check = |path: String, expr: &str| {
            if let Err(e) = engine.check(expr) {
                issues.push(ValidationIssue { severity: Severity::Error, path, message: e.to_string() });
            }
        };
        for g in &def.groups {
            check(format!("groups[{}]", g.name), &g.expression);
        }
        for b in &def.bands {
            if let Some(v) = &b.visible {
                check(format!("bands[{}].visible", b.id), v);
            }
            for el in &b.elements {
                let p = format!("bands[{}].elements[{}]", b.id, el.id);
                for expr in [&el.visible, &el.color_expression, &el.background_expression].into_iter().flatten() {
                    check(p.clone(), expr);
                }
                match &el.kind {
                    ElementKind::Expression { expression, .. } => check(p.clone(), expression),
                    ElementKind::Table(t) => {
                        for c in &t.columns {
                            check(format!("{p}.columns[{}]", c.header), &c.expression);
                            if let Some(f) = &c.footer {
                                check(format!("{p}.columns[{}].footer", c.header), f);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        issues
    }

    pub fn check_expression(&self, expr: &str) -> Result<(), String> {
        FormulaEngine::new(&Default::default(), "").check(expr).map_err(|e| e.to_string())
    }
}

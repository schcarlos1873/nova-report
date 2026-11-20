//! Motor de fórmulas sandboxed baseado em Rhai.
//!
//! Variáveis disponíveis nas expressões:
//! * `Fields.<coluna>` — linha corrente do dataset;
//! * `Params.<nome>` — parâmetros do relatório (array quando o parâmetro aceita vários valores);
//! * `Group.Sum_<coluna>` / `Group.Count` — agregados do escopo atual (grupo ou relatório);
//! * `Report.Title`, `Report.RecordCount`;
//! * `Format.currency(v, "BRL")`, `Format.number(v, 2)`, `Format.percent(v, 1)`, `Format.date(v, fmt)`.
//!
//! Funções: `SUM`, `AVG`, `MIN`, `MAX`, `COUNT`, `DISTINCT_COUNT`, `RUNNING_SUM`, `IIF`/`IFF`,
//! `FORMAT_DATE`, `FormatBRL`, `ROUND`, `ABS`, `UPPER`, `LOWER`, `TRIM`, `LEFT`, `RIGHT`, `LEN`,
//! `ISNULL`, `COALESCE`, `TO_NUMBER`, `TODAY`, `NOW`, `RECORD_NUMBER`, `PAGE_NUMBER`, `TOTAL_PAGES`,
//! `JOIN(lista, separador)` e `CONTAINS(lista, valor)` para parâmetros de vários valores.

pub mod format;
mod preprocess;

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use nova_domain::ParamValue;
use polars::prelude::DataFrame;
use rhai::{AST, Dynamic, Engine, ImmutableString, Map, Scope};

use crate::data::CellValue;
use crate::{EngineError, EngineResult};

pub use preprocess::preprocess;

/// Marcadores substituídos pelo transpilador por contadores de página do Typst.
pub const PAGE_NUMBER_MARK: &str = "\u{E000}PAGE\u{E001}";
pub const TOTAL_PAGES_MARK: &str = "\u{E000}PAGES\u{E001}";

/// Estado compartilhado entre o escopo de avaliação e as funções nativas.
#[derive(Default)]
struct EvalState {
    dataset: Option<DataFrame>,
    /// Fatia corrente (início, tamanho) — grupo ou relatório inteiro.
    range: (usize, usize),
    row: Option<usize>,
}

impl EvalState {
    fn column_values(&self, field: &str, upto_row: bool) -> Vec<CellValue> {
        let Some(df) = &self.dataset else { return vec![] };
        let Ok(col) = df.column(field) else { return vec![] };
        let (start, len) = self.range;
        let end = if upto_row { self.row.map_or(start + len, |r| r + 1) } else { start + len };
        (start..end.min(df.height()))
            .filter_map(|i| col.get(i).ok().map(|av| CellValue::from_any(&av)))
            .collect()
    }

    fn numbers(&self, field: &str) -> Vec<f64> {
        self.column_values(field, false).iter().filter_map(CellValue::as_f64).collect()
    }

    /// MIN/MAX: numérico quando todos os valores são números; senão compara datas
    /// cronologicamente e textos lexicograficamente.
    fn extreme(&self, field: &str, want: Ordering) -> Dynamic {
        let vals: Vec<CellValue> = self.column_values(field, false).into_iter().filter(|v| !v.is_null()).collect();
        if vals.is_empty() {
            return 0.0.into();
        }
        if let Some(nums) = vals.iter().map(CellValue::as_f64).collect::<Option<Vec<_>>>() {
            let pick = if want == Ordering::Less { f64::min } else { f64::max };
            return nums.into_iter().reduce(pick).unwrap_or(0.0).into();
        }
        vals.iter()
            .reduce(|best, v| if compare_cells(v, best) == want { v } else { best })
            .map_or(Dynamic::UNIT, cell_to_dynamic)
    }
}

fn compare_cells(a: &CellValue, b: &CellValue) -> Ordering {
    fn as_datetime(v: &CellValue) -> Option<chrono::NaiveDateTime> {
        match v {
            CellValue::Date(d) => d.and_hms_opt(0, 0, 0),
            CellValue::DateTime(d) => Some(*d),
            _ => None,
        }
    }
    match (as_datetime(a), as_datetime(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => a.to_display().cmp(&b.to_display()),
    }
}

#[derive(Clone)]
struct FormatNs;

pub struct FormulaEngine {
    engine: Engine,
    cache: HashMap<String, AST>,
    state: Rc<RefCell<EvalState>>,
    fields: Map,
    group: Map,
    params: Map,
    report: Map,
}

impl FormulaEngine {
    pub fn new(parameters: &HashMap<String, ParamValue>, report_title: &str) -> Self {
        let state = Rc::new(RefCell::new(EvalState::default()));
        let mut engine = Engine::new();
        engine.set_max_operations(50_000);
        engine.set_max_expr_depths(32, 32);
        engine.set_max_string_size(1 << 20);
        engine.set_max_array_size(10_000);
        engine.set_max_map_size(10_000);
        engine.set_max_call_levels(16);
        engine.disable_symbol("eval");
        engine.on_print(|_| {});
        engine.on_debug(|_, _, _| {});
        register_builtins(&mut engine, &state);

        let params = parameters
            .iter()
            .map(|(k, v)| (k.as_str().into(), param_to_dynamic(v)))
            .collect();
        let mut report = Map::new();
        report.insert("Title".into(), report_title.to_string().into());
        report.insert("RecordCount".into(), Dynamic::from(0_i64));

        Self { engine, cache: HashMap::new(), state, fields: Map::new(), group: Map::new(), params, report }
    }

    /// Valida a sintaxe de uma expressão sem executá-la.
    pub fn check(&self, expr: &str) -> EngineResult<()> {
        self.engine
            .compile(preprocess(expr))
            .map(|_| ())
            .map_err(|e| EngineError::Expression { expression: expr.into(), message: e.to_string() })
    }

    /// Define o dataset corrente (detalhe principal ou dataset de uma tabela).
    pub fn set_dataset(&mut self, df: Option<DataFrame>) {
        let height = df.as_ref().map_or(0, |d| d.height());
        self.report.insert("RecordCount".into(), Dynamic::from(height as i64));
        let mut st = self.state.borrow_mut();
        st.dataset = df;
        st.range = (0, height);
        st.row = None;
        drop(st);
        self.refresh_group();
        self.fields.clear();
    }

    /// Posição corrente (fatia, linha) para salvar/restaurar o contexto.
    pub fn position(&self) -> ((usize, usize), Option<usize>) {
        let st = self.state.borrow();
        (st.range, st.row)
    }

    pub fn dataset_height(&self) -> usize {
        self.state.borrow().dataset.as_ref().map_or(0, |d| d.height())
    }

    /// Define a fatia de agregação (grupo atual ou relatório inteiro).
    pub fn set_range(&mut self, start: usize, len: usize) {
        let changed = self.state.borrow().range != (start, len);
        if changed {
            self.state.borrow_mut().range = (start, len);
            self.refresh_group();
        }
    }

    pub fn set_full_range(&mut self) {
        let h = self.dataset_height();
        self.set_range(0, h);
    }

    /// Posiciona na linha `row` e materializa o mapa `Fields`.
    pub fn set_row(&mut self, row: Option<usize>) {
        self.state.borrow_mut().row = row;
        self.fields.clear();
        let st = self.state.borrow();
        if let (Some(df), Some(r)) = (&st.dataset, row) {
            if r < df.height() {
                for col in df.columns() {
                    if let Ok(av) = col.get(r) {
                        self.fields.insert(col.name().as_str().into(), cell_to_dynamic(&CellValue::from_any(&av)));
                    }
                }
            }
        }
    }

    fn refresh_group(&mut self) {
        self.group.clear();
        let st = self.state.borrow();
        let (_, len) = st.range;
        self.group.insert("Count".into(), Dynamic::from(len as i64));
        if let Some(df) = &st.dataset {
            for col in df.columns() {
                if col.dtype().is_primitive_numeric() {
                    let sum: f64 = st.numbers(col.name().as_str()).iter().sum();
                    self.group.insert(format!("Sum_{}", col.name()).into(), Dynamic::from(sum));
                }
            }
        }
    }

    /// Avalia uma expressão no contexto corrente.
    pub fn eval(&mut self, expr: &str) -> EngineResult<Dynamic> {
        if !self.cache.contains_key(expr) {
            let ast = self
                .engine
                .compile(preprocess(expr))
                .map_err(|e| EngineError::Expression { expression: expr.into(), message: e.to_string() })?;
            self.cache.insert(expr.to_string(), ast);
        }
        let ast = &self.cache[expr];
        let mut scope = Scope::new();
        scope.push_constant("Fields", self.fields.clone());
        scope.push_constant("Params", self.params.clone());
        scope.push_constant("Group", self.group.clone());
        scope.push_constant("Report", self.report.clone());
        scope.push("Format", FormatNs);
        self.engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, ast)
            .map_err(|e| EngineError::Expression { expression: expr.into(), message: e.to_string() })
    }

    pub fn eval_bool(&mut self, expr: &str) -> EngineResult<bool> {
        Ok(truthy(&self.eval(expr)?))
    }

    pub fn eval_string(&mut self, expr: &str) -> EngineResult<String> {
        Ok(dynamic_to_string(&self.eval(expr)?))
    }

    /// Avalia e aplica a especificação de formato opcional.
    pub fn eval_formatted(&mut self, expr: &str, spec: Option<&str>) -> EngineResult<String> {
        let value = self.eval(expr)?;
        let text = dynamic_to_string(&value);
        Ok(match spec {
            Some(s) if !s.trim().is_empty() => format::apply_spec(&text, dynamic_to_f64(&value), s),
            _ => text,
        })
    }

    /// Valor bruto usado para chaves de grupo.
    pub fn eval_key(&mut self, expr: &str) -> EngineResult<String> {
        Ok(format!("{:?}", self.eval(expr)?))
    }

    /// Nomes de campos referenciados (`Fields.x`) — útil para diagnósticos no designer.
    pub fn referenced_fields(expr: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut rest = expr;
        while let Some(pos) = rest.find("Fields.") {
            let tail = &rest[pos + 7..];
            let name: String = tail.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                out.insert(name);
            }
            rest = tail;
        }
        out
    }
}

pub fn truthy(v: &Dynamic) -> bool {
    if let Ok(b) = v.as_bool() {
        return b;
    }
    if v.is_unit() {
        return false;
    }
    if let Ok(i) = v.as_int() {
        return i != 0;
    }
    if let Ok(f) = v.as_float() {
        return f != 0.0;
    }
    v.clone().into_immutable_string().map(|s| !s.is_empty()).unwrap_or(true)
}

pub fn dynamic_to_string(v: &Dynamic) -> String {
    if v.is_unit() {
        String::new()
    } else if let Ok(f) = v.as_float() {
        format::plain_float(f)
    } else {
        v.to_string()
    }
}

pub fn dynamic_to_f64(v: &Dynamic) -> Option<f64> {
    v.as_float()
        .ok()
        .or_else(|| v.as_int().ok().map(|i| i as f64))
        .or_else(|| v.clone().into_immutable_string().ok().and_then(|s| s.trim().parse().ok()))
}

fn cell_to_dynamic(v: &CellValue) -> Dynamic {
    match v {
        CellValue::Null => Dynamic::UNIT,
        CellValue::Bool(b) => (*b).into(),
        CellValue::Int(i) => (*i).into(),
        CellValue::Float(f) => (*f).into(),
        other => other.to_display().into(),
    }
}

fn param_to_dynamic(v: &ParamValue) -> Dynamic {
    match v {
        ParamValue::Int(i) => (*i).into(),
        ParamValue::Float(f) => (*f).into(),
        ParamValue::Bool(b) => (*b).into(),
        ParamValue::Null => Dynamic::UNIT,
        ParamValue::List(v) => Dynamic::from_array(v.iter().map(param_to_dynamic).collect()),
        other => other.to_text().into(),
    }
}

fn num(v: &Dynamic) -> f64 {
    dynamic_to_f64(v).unwrap_or(0.0)
}

fn register_builtins(engine: &mut Engine, state: &Rc<RefCell<EvalState>>) {
    // ---- agregações sobre a fatia corrente
    let s = state.clone();
    engine.register_fn("SUM", move |f: &str| -> f64 { s.borrow().numbers(f).iter().sum() });
    let s = state.clone();
    engine.register_fn("AVG", move |f: &str| -> f64 {
        let v = s.borrow().numbers(f);
        if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
    });
    let s = state.clone();
    engine.register_fn("MIN", move |f: &str| -> Dynamic { s.borrow().extreme(f, Ordering::Less) });
    let s = state.clone();
    engine.register_fn("MAX", move |f: &str| -> Dynamic { s.borrow().extreme(f, Ordering::Greater) });
    let s = state.clone();
    engine.register_fn("COUNT", move || -> i64 { s.borrow().range.1 as i64 });
    let s = state.clone();
    engine.register_fn("COUNT", move |f: &str| -> i64 {
        s.borrow().column_values(f, false).iter().filter(|v| !v.is_null()).count() as i64
    });
    let s = state.clone();
    engine.register_fn("DISTINCT_COUNT", move |f: &str| -> i64 {
        let vals = s.borrow().column_values(f, false);
        vals.iter().map(CellValue::to_display).collect::<HashSet<_>>().len() as i64
    });
    let s = state.clone();
    engine.register_fn("RUNNING_SUM", move |f: &str| -> f64 {
        s.borrow().column_values(f, true).iter().filter_map(CellValue::as_f64).sum()
    });
    let s = state.clone();
    engine.register_fn("RECORD_NUMBER", move || -> i64 {
        let st = s.borrow();
        st.row.map_or(0, |r| (r - st.range.0.min(r)) as i64 + 1)
    });

    // ---- paginação (resolvida pelo Typst)
    engine.register_fn("PAGE_NUMBER", || PAGE_NUMBER_MARK.to_string());
    engine.register_fn("TOTAL_PAGES", || TOTAL_PAGES_MARK.to_string());

    // ---- condicionais
    for name in ["IIF", "IFF"] {
        engine.register_fn(name, |c: Dynamic, t: Dynamic, f: Dynamic| if truthy(&c) { t } else { f });
    }
    engine.register_fn("ISNULL", |v: Dynamic| v.is_unit());
    engine.register_fn("COALESCE", |a: Dynamic, b: Dynamic| if a.is_unit() { b } else { a });

    // ---- formatação
    engine.register_fn("FORMAT_DATE", |v: Dynamic, fmt: &str| format::date(&dynamic_to_string(&v), fmt));
    engine.register_fn("FormatBRL", |v: Dynamic| format::currency(num(&v), "BRL"));
    engine.register_fn("FORMAT_NUMBER", |v: Dynamic, d: i64| format::number_br(num(&v), d.max(0) as usize));
    engine.register_type_with_name::<FormatNs>("Format");
    engine.register_fn("currency", |_: FormatNs, v: Dynamic, code: &str| format::currency(num(&v), code));
    engine.register_fn("currency", |_: FormatNs, v: Dynamic| format::currency(num(&v), "BRL"));
    engine.register_fn("number", |_: FormatNs, v: Dynamic, d: i64| format::number_br(num(&v), d.max(0) as usize));
    engine.register_fn("percent", |_: FormatNs, v: Dynamic, d: i64| format::percent(num(&v), d.max(0) as usize));
    engine.register_fn("date", |_: FormatNs, v: Dynamic, fmt: &str| format::date(&dynamic_to_string(&v), fmt));

    // ---- números e textos
    engine.register_fn("ROUND", |v: Dynamic, d: i64| {
        let m = 10f64.powi(d.clamp(0, 12) as i32);
        (num(&v) * m).round() / m
    });
    engine.register_fn("ABS", |v: Dynamic| num(&v).abs());
    engine.register_fn("TO_NUMBER", |v: Dynamic| num(&v));
    engine.register_fn("TO_STRING", |v: Dynamic| dynamic_to_string(&v));
    engine.register_fn("UPPER", |v: Dynamic| dynamic_to_string(&v).to_uppercase());
    engine.register_fn("LOWER", |v: Dynamic| dynamic_to_string(&v).to_lowercase());
    engine.register_fn("TRIM", |v: Dynamic| dynamic_to_string(&v).trim().to_string());
    engine.register_fn("LEN", |v: Dynamic| dynamic_to_string(&v).chars().count() as i64);
    // ---- parâmetros de vários valores
    engine.register_fn("JOIN", |v: Dynamic, sep: &str| -> String {
        match v.into_array() {
            Ok(a) => a.iter().map(dynamic_to_string).collect::<Vec<_>>().join(sep),
            Err(_) => String::new(),
        }
    });
    engine.register_fn("CONTAINS", |list: Dynamic, item: Dynamic| -> bool {
        let needle = dynamic_to_string(&item);
        match list.into_array() {
            Ok(a) => a.iter().any(|x| dynamic_to_string(x) == needle),
            Err(_) => false,
        }
    });
    engine.register_fn("LEFT", |v: Dynamic, n: i64| -> ImmutableString {
        dynamic_to_string(&v).chars().take(n.max(0) as usize).collect::<String>().into()
    });
    engine.register_fn("RIGHT", |v: Dynamic, n: i64| -> ImmutableString {
        let s: Vec<char> = dynamic_to_string(&v).chars().collect();
        s[s.len().saturating_sub(n.max(0) as usize)..].iter().collect::<String>().into()
    });
    engine.register_fn("TODAY", || chrono::Local::now().format("%Y-%m-%d").to_string());
    engine.register_fn("NOW", || chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
}

#[cfg(test)]
mod tests;

//! Reescrita de parâmetros nomeados (`:nome`) para o placeholder de cada driver.

use std::collections::HashMap;

use nova_domain::ParamValue;
use nova_domain::report::sql::{SqlToken, tokenize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderStyle {
    /// PostgreSQL: `$1`, `$2` (reutiliza índice para o mesmo nome)
    Dollar,
    /// MySQL / ODBC: `?` (valor repetido a cada ocorrência)
    Question,
    /// SQL Server (tiberius): `@P1`
    AtP,
    /// SQLite: `?1`
    QuestionNumbered,
}

/// Retorna o SQL reescrito e os valores na ordem dos placeholders.
///
/// Parâmetros de valores múltiplos (`ParamValue::List`) viram um placeholder por elemento,
/// separados por vírgula — `x IN (:ids)` → `x IN (@P1, @P2)`. Lista vazia vira `NULL`
/// (`IN (NULL)` não retorna linhas). Os valores devolvidos são sempre escalares.
pub fn bind_named(sql: &str, params: &HashMap<String, ParamValue>, style: PlaceholderStyle) -> (String, Vec<ParamValue>) {
    let mut out = String::with_capacity(sql.len());
    let mut values: Vec<ParamValue> = Vec::new();
    // nome → índices (1-based) já atribuídos, para reutilizar em estilos numerados
    let mut index_of: HashMap<&str, Vec<usize>> = HashMap::new();
    let placeholder = |i: usize| match style {
        PlaceholderStyle::Dollar => format!("${i}"),
        PlaceholderStyle::AtP => format!("@P{i}"),
        PlaceholderStyle::QuestionNumbered => format!("?{i}"),
        PlaceholderStyle::Question => "?".to_string(),
    };
    for tok in tokenize(sql) {
        match tok {
            SqlToken::Text(t) => out.push_str(t),
            SqlToken::Param(name) => {
                let value = params.get(name).cloned().unwrap_or(ParamValue::Null);
                let scalars = match value {
                    ParamValue::List(v) if v.is_empty() => {
                        out.push_str("NULL");
                        continue;
                    }
                    ParamValue::List(v) => v,
                    other => vec![other],
                };
                let indices = if style == PlaceholderStyle::Question {
                    // `?` posicional: o valor é repetido a cada ocorrência
                    scalars.into_iter().map(|v| { values.push(v); values.len() }).collect()
                } else {
                    index_of
                        .entry(name)
                        .or_insert_with(|| scalars.into_iter().map(|v| { values.push(v); values.len() }).collect())
                        .clone()
                };
                out.push_str(&indices.iter().map(|&i| placeholder(i)).collect::<Vec<_>>().join(", "));
            }
        }
    }
    (out, values)
}

/// Literal SQL escapado de um valor escalar.
fn literal(v: &ParamValue) -> String {
    match v {
        ParamValue::Null => "NULL".into(),
        ParamValue::Int(i) => i.to_string(),
        ParamValue::Float(f) if f.is_finite() => f.to_string(),
        ParamValue::Float(_) => "NULL".into(),
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::List(v) if v.is_empty() => "NULL".into(),
        ParamValue::List(v) => v.iter().map(literal).collect::<Vec<_>>().join(", "),
        other => format!("'{}'", other.to_text().replace('\'', "''")),
    }
}

/// Substitui parâmetros por literais escapados (para motores sem bind, ex.: Polars SQL).
pub fn inline_literals(sql: &str, params: &HashMap<String, ParamValue>) -> String {
    tokenize(sql)
        .into_iter()
        .map(|t| match t {
            SqlToken::Text(s) => s.to_string(),
            SqlToken::Param(name) => literal(params.get(name).unwrap_or(&ParamValue::Null)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> HashMap<String, ParamValue> {
        HashMap::from([("a".into(), ParamValue::Int(1)), ("b".into(), ParamValue::Text("o'k".into()))])
    }

    #[test]
    fn rewrites_per_dialect() {
        let sql = "SELECT * FROM t WHERE x = :a AND y = :b OR z = :a";
        let (pg, v) = bind_named(sql, &params(), PlaceholderStyle::Dollar);
        assert_eq!(pg, "SELECT * FROM t WHERE x = $1 AND y = $2 OR z = $1");
        assert_eq!(v.len(), 2);
        let (my, v) = bind_named(sql, &params(), PlaceholderStyle::Question);
        assert_eq!(my, "SELECT * FROM t WHERE x = ? AND y = ? OR z = ?");
        assert_eq!(v.len(), 3);
        let (ms, _) = bind_named(sql, &params(), PlaceholderStyle::AtP);
        assert_eq!(ms, "SELECT * FROM t WHERE x = @P1 AND y = @P2 OR z = @P1");
    }

    #[test]
    fn expands_multi_value_lists() {
        let mut p = params();
        p.insert("ids".into(), ParamValue::List(vec![ParamValue::Int(7), ParamValue::Int(9)]));
        p.insert("vazio".into(), ParamValue::List(vec![]));
        let sql = "WHERE id IN (:ids) AND a = :a AND id2 IN (:ids) AND z IN (:vazio)";
        let (ms, v) = bind_named(sql, &p, PlaceholderStyle::AtP);
        assert_eq!(ms, "WHERE id IN (@P1, @P2) AND a = @P3 AND id2 IN (@P1, @P2) AND z IN (NULL)");
        assert_eq!(v, vec![ParamValue::Int(7), ParamValue::Int(9), ParamValue::Int(1)]);
        let (my, v) = bind_named(sql, &p, PlaceholderStyle::Question);
        assert_eq!(my, "WHERE id IN (?, ?) AND a = ? AND id2 IN (?, ?) AND z IN (NULL)");
        assert_eq!(v.len(), 5);
        let (lite, _) = bind_named("x IN (:ids)", &p, PlaceholderStyle::QuestionNumbered);
        assert_eq!(lite, "x IN (?1, ?2)");
        assert_eq!(inline_literals("x IN (:ids) OR y IN (:vazio)", &p), "x IN (7, 9) OR y IN (NULL)");
        p.insert("t".into(), ParamValue::List(vec![ParamValue::Text("o'k".into())]));
        assert_eq!(inline_literals("x IN (:t)", &p), "x IN ('o''k')");
    }

    #[test]
    fn inline_escapes_quotes() {
        assert_eq!(inline_literals("v = :b", &params()), "v = 'o''k'");
    }
}

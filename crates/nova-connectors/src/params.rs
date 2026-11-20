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
pub fn bind_named(sql: &str, params: &HashMap<String, ParamValue>, style: PlaceholderStyle) -> (String, Vec<ParamValue>) {
    let mut out = String::with_capacity(sql.len());
    let mut values: Vec<ParamValue> = Vec::new();
    let mut index_of: HashMap<&str, usize> = HashMap::new();
    for tok in tokenize(sql) {
        match tok {
            SqlToken::Text(t) => out.push_str(t),
            SqlToken::Param(name) => {
                let value = params.get(name).cloned().unwrap_or(ParamValue::Null);
                match style {
                    PlaceholderStyle::Question => {
                        values.push(value);
                        out.push('?');
                    }
                    _ => {
                        let idx = *index_of.entry(name).or_insert_with(|| {
                            values.push(value);
                            values.len()
                        });
                        match style {
                            PlaceholderStyle::Dollar => out.push_str(&format!("${idx}")),
                            PlaceholderStyle::AtP => out.push_str(&format!("@P{idx}")),
                            _ => out.push_str(&format!("?{idx}")),
                        }
                    }
                }
            }
        }
    }
    (out, values)
}

/// Substitui parâmetros por literais escapados (para motores sem bind, ex.: Polars SQL).
pub fn inline_literals(sql: &str, params: &HashMap<String, ParamValue>) -> String {
    tokenize(sql)
        .into_iter()
        .map(|t| match t {
            SqlToken::Text(s) => s.to_string(),
            SqlToken::Param(name) => match params.get(name).unwrap_or(&ParamValue::Null) {
                ParamValue::Null => "NULL".into(),
                ParamValue::Int(i) => i.to_string(),
                ParamValue::Float(f) if f.is_finite() => f.to_string(),
                ParamValue::Float(_) => "NULL".into(),
                ParamValue::Bool(b) => b.to_string(),
                other => format!("'{}'", other.to_text().replace('\'', "''")),
            },
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
    fn inline_escapes_quotes() {
        assert_eq!(inline_literals("v = :b", &params()), "v = 'o''k'");
    }
}

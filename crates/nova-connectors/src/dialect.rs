//! Abstração de dialeto SQL (quoting, paginação, literais de data).

use nova_domain::report::ProviderKind;

pub trait SqlDialect: Send + Sync {
    fn quote_identifier(&self, id: &str) -> String;
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String;
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String;

    /// `SELECT * FROM schema.tabela` para pré-visualização no navegador de schema.
    fn select_all(&self, schema: Option<&str>, table: &str) -> String {
        match schema {
            Some(s) if !s.is_empty() && s != "main" => {
                format!("SELECT * FROM {}.{}", self.quote_identifier(s), self.quote_identifier(table))
            }
            _ => format!("SELECT * FROM {}", self.quote_identifier(table)),
        }
    }
}

fn quote_with(id: &str, open: char, close: char) -> String {
    let escaped = id.replace(close, &format!("{close}{close}"));
    format!("{open}{escaped}{close}")
}

/// Remove `;` final para permitir envolver a query em subselect. O que é anexado depois começa
/// em nova linha para não cair dentro de um comentário `--` final.
fn strip(query: &str) -> &str {
    query.trim().trim_end_matches(';')
}

/// Cópia da query em maiúsculas com strings, identificadores delimitados, comentários e o
/// conteúdo de parênteses trocados por espaços: o que sobra são só as cláusulas do nível
/// principal (posições preservadas em bytes ASCII).
fn top_level_mask(query: &str) -> String {
    let b = query.as_bytes();
    let mut out = vec![b' '; b.len()];
    let (mut i, mut depth) = (0, 0usize);
    while i < b.len() {
        let c = b[i];
        let skip_to = |end: &[u8], from: usize| -> usize {
            b[from..].windows(end.len()).position(|w| w == end).map_or(b.len(), |p| from + p + end.len())
        };
        match c {
            b'\'' | b'"' | b'[' => {
                let close = if c == b'[' { b']' } else { c };
                i += 1;
                while i < b.len() {
                    if b[i] == close {
                        // aspas duplicadas são escape (`'it''s'`, `[a]]b]`)
                        if b.get(i + 1) == Some(&close) { i += 2; continue; }
                        break;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'-' if b.get(i + 1) == Some(&b'-') => i = skip_to(b"\n", i),
            b'/' if b.get(i + 1) == Some(&b'*') => i = skip_to(b"*/", i + 2),
            b'(' => { depth += 1; i += 1; }
            b')' => { depth = depth.saturating_sub(1); i += 1; }
            _ => {
                if depth == 0 && c.is_ascii() { out[i] = c.to_ascii_uppercase(); }
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Há a sequência de palavras (separadas por espaços) no nível principal da query?
fn has_top_level(mask: &str, words: &[&str]) -> bool {
    let tokens: Vec<&str> = mask.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).filter(|t| !t.is_empty()).collect();
    tokens.windows(words.len()).any(|w| w == words)
}

pub struct PostgresDialect;
impl SqlDialect for PostgresDialect {
    fn quote_identifier(&self, id: &str) -> String {
        quote_with(id, '"', '"')
    }
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        format!("SELECT * FROM ({}\n) AS _nr LIMIT {limit} OFFSET {offset}", strip(query))
    }
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String {
        format!("'{}'::date", date.format("%Y-%m-%d"))
    }
}

pub struct MsSqlDialect;
impl SqlDialect for MsSqlDialect {
    fn quote_identifier(&self, id: &str) -> String {
        quote_with(id, '[', ']')
    }
    /// O SQL Server não aceita `ORDER BY` nem CTE (`WITH`) dentro de subconsulta, então nesses
    /// casos a paginação é anexada à própria query em vez de envolvê-la.
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        let q = strip(query);
        let mask = top_level_mask(q);
        let page = format!("OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY");
        let first = mask.split_whitespace().next().unwrap_or("");
        if has_top_level(&mask, &["OFFSET"]) || has_top_level(&mask, &["FOR", "XML"]) || has_top_level(&mask, &["FOR", "JSON"])
            || matches!(first, "EXEC" | "EXECUTE" | "DECLARE")
        {
            // já paginada / formato especial / lote T-SQL: executa como está (a prévia corta as linhas)
            q.to_string()
        } else if has_top_level(&mask, &["ORDER", "BY"]) {
            format!("{q}\n{page}")
        } else if first == "WITH" {
            format!("{q}\nORDER BY (SELECT NULL) {page}")
        } else {
            format!("SELECT * FROM ({q}\n) AS _nr ORDER BY (SELECT NULL) {page}")
        }
    }
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String {
        format!("'{}'", date.format("%Y-%m-%d"))
    }
}

pub struct MySqlDialect;
impl SqlDialect for MySqlDialect {
    fn quote_identifier(&self, id: &str) -> String {
        quote_with(id, '`', '`')
    }
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        format!("SELECT * FROM ({}\n) AS _nr LIMIT {limit} OFFSET {offset}", strip(query))
    }
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String {
        format!("'{}'", date.format("%Y-%m-%d"))
    }
}

pub struct SqliteDialect;
impl SqlDialect for SqliteDialect {
    fn quote_identifier(&self, id: &str) -> String {
        quote_with(id, '"', '"')
    }
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        format!("SELECT * FROM ({}\n) LIMIT {limit} OFFSET {offset}", strip(query))
    }
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String {
        format!("'{}'", date.format("%Y-%m-%d"))
    }
}

/// Oracle / DB2 (via ODBC) — sintaxe ANSI `OFFSET ... FETCH`.
pub struct AnsiDialect;
impl SqlDialect for AnsiDialect {
    fn quote_identifier(&self, id: &str) -> String {
        quote_with(id, '"', '"')
    }
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        format!("SELECT * FROM ({}\n) _nr OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY", strip(query))
    }
    fn format_date_literal(&self, date: chrono::NaiveDate) -> String {
        format!("DATE '{}'", date.format("%Y-%m-%d"))
    }
}

pub fn for_provider(kind: ProviderKind) -> &'static dyn SqlDialect {
    match kind {
        ProviderKind::Postgres => &PostgresDialect,
        ProviderKind::Mssql => &MsSqlDialect,
        ProviderKind::Mysql => &MySqlDialect,
        ProviderKind::Odbc => &AnsiDialect,
        _ => &SqliteDialect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_escapes_closing_char() {
        assert_eq!(MsSqlDialect.quote_identifier("a]b"), "[a]]b]");
        assert_eq!(PostgresDialect.quote_identifier("a\"b"), "\"a\"\"b\"");
        assert_eq!(MySqlDialect.select_all(Some("erp"), "pedidos"), "SELECT * FROM `erp`.`pedidos`");
    }

    #[test]
    fn mssql_pagination_keeps_order_by_outside_subquery() {
        let q = "select mf.Id, mf.Code from MastFactory mf where mf.IsActive = 1 ORDER BY mf.Code";
        assert_eq!(MsSqlDialect.apply_pagination(q, 100, 0), format!("{q}\nOFFSET 0 ROWS FETCH NEXT 100 ROWS ONLY"));
        // sem ORDER BY principal continua envolvendo (ORDER BY de subconsulta/string não conta)
        let q = "SELECT a FROM (SELECT TOP 5 a FROM t ORDER BY a) x WHERE b = 'order by'";
        assert_eq!(
            MsSqlDialect.apply_pagination(q, 10, 0),
            format!("SELECT * FROM ({q}\n) AS _nr ORDER BY (SELECT NULL) OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY")
        );
        // CTE não pode ir para subconsulta
        let q = "WITH x AS (SELECT 1 AS a) SELECT a FROM x";
        assert_eq!(MsSqlDialect.apply_pagination(q, 1, 0), format!("{q}\nORDER BY (SELECT NULL) OFFSET 0 ROWS FETCH NEXT 1 ROWS ONLY"));
        // já paginada, FOR XML ou procedure: inalterada
        for q in ["SELECT a FROM t ORDER BY a OFFSET 5 ROWS", "SELECT a FROM t ORDER BY a FOR XML PATH", "EXEC dbo.rel @x = 1"] {
            assert_eq!(MsSqlDialect.apply_pagination(q, 10, 0), q);
        }
        // comentário com ORDER BY não engana
        let q = "SELECT a FROM t -- ORDER BY a";
        assert!(MsSqlDialect.apply_pagination(q, 10, 0).starts_with("SELECT * FROM ("));
        assert!(MsSqlDialect.apply_pagination("SELECT a FROM t ORDER BY a -- fim", 10, 0).ends_with("-- fim\nOFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"));
    }

    #[test]
    fn pagination_wraps_query() {
        assert_eq!(
            PostgresDialect.apply_pagination("SELECT 1;", 10, 20),
            "SELECT * FROM (SELECT 1\n) AS _nr LIMIT 10 OFFSET 20"
        );
    }
}

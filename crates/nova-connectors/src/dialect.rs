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

/// Remove `;` final para permitir envolver a query em subselect.
fn strip(query: &str) -> &str {
    query.trim().trim_end_matches(';')
}

pub struct PostgresDialect;
impl SqlDialect for PostgresDialect {
    fn quote_identifier(&self, id: &str) -> String {
        quote_with(id, '"', '"')
    }
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        format!("SELECT * FROM ({}) AS _nr LIMIT {limit} OFFSET {offset}", strip(query))
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
    fn apply_pagination(&self, query: &str, limit: usize, offset: usize) -> String {
        format!("SELECT * FROM ({}) AS _nr ORDER BY (SELECT NULL) OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY", strip(query))
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
        format!("SELECT * FROM ({}) AS _nr LIMIT {limit} OFFSET {offset}", strip(query))
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
        format!("SELECT * FROM ({}) LIMIT {limit} OFFSET {offset}", strip(query))
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
        format!("SELECT * FROM ({}) _nr OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY", strip(query))
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
    fn pagination_wraps_query() {
        assert_eq!(
            PostgresDialect.apply_pagination("SELECT 1;", 10, 20),
            "SELECT * FROM (SELECT 1) AS _nr LIMIT 10 OFFSET 20"
        );
    }
}

//! Serviço de domínio para parâmetros nomeados em SQL (`:nome`).
//!
//! O scanner ignora literais de string, identificadores entre aspas, comentários e
//! casts do PostgreSQL (`::date`), e é compartilhado entre validação e drivers.

/// Um trecho do SQL: texto literal ou referência a parâmetro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlToken<'a> {
    Text(&'a str),
    Param(&'a str),
}

pub fn tokenize(sql: &str) -> Vec<SqlToken<'_>> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut start = 0;
    while i < bytes.len() {
        match bytes[i] {
            q @ (b'\'' | b'"' | b'`') => {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == q {
                        // aspas duplicadas = escape
                        if i + 1 < bytes.len() && bytes[i + 1] == q {
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'[' => {
                // identificador MSSQL
                while i < bytes.len() && bytes[i] != b']' {
                    i += 1;
                }
                i += 1;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b':' if bytes.get(i + 1) == Some(&b':') => i += 2,
            b':' if bytes.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_') => {
                if start < i {
                    out.push(SqlToken::Text(&sql[start..i]));
                }
                let name_start = i + 1;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                out.push(SqlToken::Param(&sql[name_start..i]));
                start = i;
            }
            _ => i += 1,
        }
    }
    let end = sql.len();
    if start < end {
        out.push(SqlToken::Text(&sql[start.min(end)..end]));
    }
    out
}

/// Nomes distintos de parâmetros referenciados, na ordem de aparição.
pub fn named_parameters(sql: &str) -> Vec<String> {
    let mut seen = Vec::<String>::new();
    for t in tokenize(sql) {
        if let SqlToken::Param(p) = t {
            if !seen.iter().any(|s| s == p) {
                seen.push(p.to_string());
            }
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_literals_comments_and_casts() {
        let sql = "SELECT ':x', \"a:b\", d::date -- :c\n FROM t /* :z */ WHERE a = :p1 AND b = :p_2 OR c = :p1";
        assert_eq!(named_parameters(sql), vec!["p1", "p_2"]);
    }

    #[test]
    fn roundtrip_preserves_text() {
        let sql = "SELECT * FROM t WHERE x >= :inicio AND y <= :fim";
        let rebuilt: String = tokenize(sql)
            .into_iter()
            .map(|t| match t {
                SqlToken::Text(s) => s.to_string(),
                SqlToken::Param(p) => format!(":{p}"),
            })
            .collect();
        assert_eq!(rebuilt, sql);
    }
}

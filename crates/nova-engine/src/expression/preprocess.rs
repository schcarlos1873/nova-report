/// Normaliza a sintaxe estilo Crystal/Excel para Rhai: literais entre aspas simples
/// (`'BRL'`) viram strings Rhai (`"BRL"`) e `Params.<nome>.Label` vira `ParamLabels.<nome>`.
pub fn preprocess(expr: &str) -> String {
    param_label_alias(&quotes_to_rhai(expr))
}

fn quotes_to_rhai(expr: &str) -> String {
    let mut out = String::with_capacity(expr.len() + 4);
    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' | '`' => {
                out.push(c);
                while let Some(n) = chars.next() {
                    out.push(n);
                    if n == '\\' {
                        if let Some(e) = chars.next() {
                            out.push(e);
                        }
                    } else if n == c {
                        break;
                    }
                }
            }
            '\'' => {
                out.push('"');
                while let Some(n) = chars.next() {
                    match n {
                        '\'' if chars.peek() == Some(&'\'') => {
                            chars.next();
                            out.push('\'');
                        }
                        '\'' => break,
                        '"' => out.push_str("\\\""),
                        '\\' => {
                            out.push('\\');
                            if let Some(e) = chars.next() {
                                out.push(e);
                            }
                        }
                        other => out.push(other),
                    }
                }
                out.push('"');
            }
            other => out.push(other),
        }
    }
    out
}

/// `Params.<nome>.Label` → `ParamLabels.<nome>`, fora de strings (já normalizadas para `"`/`` ` ``).
fn param_label_alias(expr: &str) -> String {
    const PREFIX: &str = "Params.";
    const SUFFIX: &str = ".Label";
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(expr.len());
    let mut i = 0;
    while i < expr.len() {
        let rest = &expr[i..];
        let c = rest.chars().next().unwrap_or_default();
        if c == '"' || c == '`' {
            // copia a string inteira, respeitando escapes
            let mut end = rest.len();
            let mut escaped = false;
            for (j, n) in rest.char_indices().skip(1) {
                if escaped {
                    escaped = false;
                } else if n == '\\' {
                    escaped = true;
                } else if n == c {
                    end = j + 1;
                    break;
                }
            }
            out.push_str(&rest[..end]);
            i += end;
            continue;
        }
        let at_boundary = !out.ends_with(|p: char| ident(p) || p == '.');
        if at_boundary && rest.starts_with(PREFIX) {
            let name_len = rest[PREFIX.len()..].find(|n: char| !ident(n)).unwrap_or(rest.len() - PREFIX.len());
            let after = &rest[PREFIX.len() + name_len..];
            if name_len > 0 && after.starts_with(SUFFIX) && !after[SUFFIX.len()..].starts_with(ident) {
                out.push_str("ParamLabels.");
                out.push_str(&rest[PREFIX.len()..PREFIX.len() + name_len]);
                i += PREFIX.len() + name_len + SUFFIX.len();
                continue;
            }
        }
        out.push(c);
        i += c.len_utf8();
    }
    out
}

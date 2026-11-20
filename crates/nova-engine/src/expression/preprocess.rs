/// Normaliza a sintaxe estilo Crystal/Excel para Rhai: literais entre aspas simples
/// (`'BRL'`) viram strings Rhai (`"BRL"`).
pub fn preprocess(expr: &str) -> String {
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

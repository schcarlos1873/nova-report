//! Formatação de valores (padrão pt-BR) usada pelas expressões e pela propriedade `format`.

use chrono::{NaiveDate, NaiveDateTime};

/// Float sem zeros à direita (até 6 casas), sem separador de milhar.
pub fn plain_float(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Número com separadores (`thousands`, `decimal`) e `decimals` casas.
pub fn number_with(v: f64, decimals: usize, thousands: char, decimal: char) -> String {
    let s = format!("{:.*}", decimals, v.abs());
    let (int_part, frac_part) = s.split_once('.').unwrap_or((&s, ""));
    let mut grouped = String::new();
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            grouped.push(thousands);
        }
        grouped.push(ch);
    }
    let sign = if v < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') { "-" } else { "" };
    if frac_part.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}{decimal}{frac_part}")
    }
}

pub fn number_br(v: f64, decimals: usize) -> String {
    number_with(v, decimals, '.', ',')
}

pub fn currency(v: f64, code: &str) -> String {
    match code.to_uppercase().as_str() {
        "BRL" => format!("R$ {}", number_br(v, 2)),
        "USD" => format!("US$ {}", number_with(v, 2, ',', '.')),
        "EUR" => format!("€ {}", number_br(v, 2)),
        "GBP" => format!("£ {}", number_with(v, 2, ',', '.')),
        other => format!("{other} {}", number_br(v, 2)),
    }
}

pub fn percent(v: f64, decimals: usize) -> String {
    format!("{}%", number_br(v * 100.0, decimals))
}

/// Formata texto ISO (`AAAA-MM-DD` ou `AAAA-MM-DD HH:MM:SS`) com padrão strftime.
pub fn date(value: &str, pattern: &str) -> String {
    let v = value.trim();
    if let Ok(dt) = NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M:%S"))
    {
        return dt.format(pattern).to_string();
    }
    if let Ok(d) = NaiveDate::parse_from_str(v.get(..10).unwrap_or(v), "%Y-%m-%d") {
        return d.format(pattern).to_string();
    }
    value.to_string()
}

/// Aplica uma especificação de formato do elemento (`currency:BRL`, `number:2`,
/// `percent:1`, `date:%d/%m/%Y`, `upper`, `lower`).
pub fn apply_spec(text: &str, numeric: Option<f64>, spec: &str) -> String {
    let (kind, arg) = spec.split_once(':').map_or((spec, ""), |(k, a)| (k, a));
    let decimals = |default: usize| arg.trim().parse().unwrap_or(default);
    match (kind.trim(), numeric) {
        ("currency", Some(n)) => currency(n, if arg.is_empty() { "BRL" } else { arg }),
        ("number", Some(n)) => number_br(n, decimals(2)),
        ("number_en", Some(n)) => number_with(n, decimals(2), ',', '.'),
        ("percent", Some(n)) => percent(n, decimals(0)),
        ("integer", Some(n)) => number_br(n.round(), 0),
        ("date" | "datetime", _) => date(text, if arg.is_empty() { "%d/%m/%Y" } else { arg }),
        ("upper", _) => text.to_uppercase(),
        ("lower", _) => text.to_lowercase(),
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brazilian_number_formatting() {
        assert_eq!(number_br(1234567.891, 2), "1.234.567,89");
        assert_eq!(number_br(-12.5, 2), "-12,50");
        assert_eq!(number_br(-0.001, 2), "0,00");
        assert_eq!(currency(1500.0, "BRL"), "R$ 1.500,00");
        assert_eq!(percent(0.256, 1), "25,6%");
        assert_eq!(plain_float(10.50), "10.5");
    }

    #[test]
    fn spec_application() {
        assert_eq!(apply_spec("2024-03-05", None, "date:%d/%m/%Y"), "05/03/2024");
        assert_eq!(apply_spec("", Some(99.9), "currency:USD"), "US$ 99.90");
    }
}

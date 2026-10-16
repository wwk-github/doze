//! GNU sleep compatible time-interval parsing.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Finite(Duration),
    Infinite,
}

/// Parses one `NUMBER[SUFFIX]` argument. `None` means the interval is invalid.
///
/// Mirrors what GNU sleep accepts: leading whitespace, optional `+`, decimal or
/// hexadecimal floating point, `inf`/`infinity`, then at most one of `smhd`.
pub fn parse_interval(arg: &str, decimal_sep: char) -> Option<Span> {
    let s = arg.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b');
    let s = s.strip_prefix('+').unwrap_or(s);
    let (seconds, rest) = parse_number(s, decimal_sep)?;
    let multiplier = match rest {
        "" | "s" => 1.0,
        "m" => 60.0,
        "h" => 3600.0,
        "d" => 86400.0,
        _ => return None,
    };
    let total = seconds * multiplier;
    if !(total >= 0.0) {
        return None;
    }
    Some(match Duration::try_from_secs_f64(total) {
        Ok(d) => Span::Finite(d),
        Err(_) => Span::Infinite,
    })
}

/// Sums intervals, saturating to `Infinite` on overflow.
pub fn sum(spans: &[Span]) -> Span {
    let mut total = Duration::ZERO;
    for span in spans {
        match span {
            Span::Infinite => return Span::Infinite,
            Span::Finite(d) => match total.checked_add(*d) {
                Some(t) => total = t,
                None => return Span::Infinite,
            },
        }
    }
    Span::Finite(total)
}

/// Parses the longest number prefix of `s`, like strtod, returning the value
/// and the unparsed remainder.
fn parse_number(s: &str, decimal_sep: char) -> Option<(f64, &str)> {
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("infinity") {
        return Some((f64::INFINITY, &s[8..]));
    }
    if lower.starts_with("inf") {
        return Some((f64::INFINITY, &s[3..]));
    }
    if lower.starts_with("0x") {
        if let Some(found) = parse_hex(&s[2..], decimal_sep) {
            return Some(found);
        }
    }
    parse_decimal(s, decimal_sep)
}

fn is_radix(c: char, decimal_sep: char) -> bool {
    c == '.' || c == decimal_sep
}

fn parse_decimal(s: &str, decimal_sep: char) -> Option<(f64, &str)> {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut i = 0;
    let mut text = String::new();
    let mut digits = 0;
    while i < chars.len() && chars[i].1.is_ascii_digit() {
        text.push(chars[i].1);
        digits += 1;
        i += 1;
    }
    if i < chars.len() && is_radix(chars[i].1, decimal_sep) {
        text.push('.');
        i += 1;
        while i < chars.len() && chars[i].1.is_ascii_digit() {
            text.push(chars[i].1);
            digits += 1;
            i += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < chars.len() && matches!(chars[i].1, 'e' | 'E') {
        let mut j = i + 1;
        let mut exponent = String::from("e");
        if j < chars.len() && matches!(chars[j].1, '+' | '-') {
            exponent.push(chars[j].1);
            j += 1;
        }
        let start = j;
        while j < chars.len() && chars[j].1.is_ascii_digit() {
            exponent.push(chars[j].1);
            j += 1;
        }
        if j > start {
            text.push_str(&exponent);
            i = j;
        }
    }
    let end = chars.get(i).map_or(s.len(), |&(pos, _)| pos);
    Some((text.parse().ok()?, &s[end..]))
}

fn parse_hex(s: &str, decimal_sep: char) -> Option<(f64, &str)> {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut i = 0;
    let mut value = 0.0f64;
    let mut scale: i64 = 0;
    let mut digits = 0;
    while i < chars.len() && chars[i].1.is_ascii_hexdigit() {
        value = value * 16.0 + chars[i].1.to_digit(16)? as f64;
        digits += 1;
        i += 1;
    }
    if i < chars.len() && is_radix(chars[i].1, decimal_sep) {
        i += 1;
        while i < chars.len() && chars[i].1.is_ascii_hexdigit() {
            value = value * 16.0 + chars[i].1.to_digit(16)? as f64;
            scale -= 4;
            digits += 1;
            i += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < chars.len() && matches!(chars[i].1, 'p' | 'P') {
        let mut j = i + 1;
        let mut negative = false;
        if j < chars.len() && matches!(chars[j].1, '+' | '-') {
            negative = chars[j].1 == '-';
            j += 1;
        }
        let start = j;
        let mut exponent: i64 = 0;
        while j < chars.len() && chars[j].1.is_ascii_digit() {
            exponent = exponent.saturating_mul(10).saturating_add(chars[j].1 as i64 - '0' as i64);
            j += 1;
        }
        if j > start {
            scale = scale.saturating_add(if negative { -exponent } else { exponent });
            i = j;
        }
    }
    let end = chars.get(i).map_or(s.len(), |&(pos, _)| pos);
    let scale = scale.clamp(-100_000, 100_000) as i32;
    let value = if value == 0.0 { 0.0 } else { value * 2f64.powi(scale) };
    Some((value, &s[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(arg: &str) -> Option<Span> {
        parse_interval(arg, '.')
    }

    fn finite(s: f64) -> Option<Span> {
        Some(Span::Finite(Duration::from_secs_f64(s)))
    }

    #[test]
    fn valid_plain() {
        assert_eq!(secs("30"), finite(30.0));
        assert_eq!(secs("1."), finite(1.0));
        assert_eq!(secs("00"), finite(0.0));
        assert_eq!(secs("+1"), finite(1.0));
        assert_eq!(secs(" 1"), finite(1.0));
        assert_eq!(secs("1e+0"), finite(1.0));
        assert_eq!(secs("1e-3"), finite(0.001));
    }

    #[test]
    fn invalid_forms() {
        for arg in ["1 ", ".", "1e", "0x", "nan", "s", "1S", "1ms", "1ss", "-0", "-1", "", "1,5"] {
            assert_eq!(secs(arg), None, "{arg:?} must be invalid");
        }
    }

    #[test]
    fn hex_float() {
        assert_eq!(secs("0x1"), finite(1.0));
        assert_eq!(secs("0X1P0"), finite(1.0));
        assert_eq!(secs("0x.8"), finite(0.5));
        assert_eq!(secs("0x1p-4"), finite(0.0625));
        assert_eq!(secs("0x0p99999"), finite(0.0));
    }

    #[test]
    fn infinity_forms() {
        for arg in ["inf", "INF", "Inf", "infinity", "infs", "infm"] {
            assert_eq!(secs(arg), Some(Span::Infinite), "{arg:?}");
        }
    }

    #[test]
    fn overflow_is_infinite() {
        assert_eq!(secs("1e400"), Some(Span::Infinite));
        assert_eq!(secs("0x1p99999"), Some(Span::Infinite));
    }

    #[test]
    fn suffixes() {
        assert_eq!(secs(".5s"), finite(0.5));
        assert_eq!(secs("2m"), finite(120.0));
        assert_eq!(secs("1h"), finite(3600.0));
        assert_eq!(secs("1.5d"), finite(129600.0));
    }

    #[test]
    fn locale_comma() {
        assert_eq!(parse_interval("1,5", ','), finite(1.5));
        assert_eq!(parse_interval("1.5", ','), finite(1.5));
        assert_eq!(parse_interval("1,5x", ','), None);
    }

    #[test]
    fn sum_saturates() {
        let max = Span::Finite(Duration::MAX);
        let one = Span::Finite(Duration::from_secs(1));
        assert_eq!(sum(&[max, one]), Span::Infinite);
    }

    #[test]
    fn sum_with_infinite() {
        let one = Span::Finite(Duration::from_secs(1));
        assert_eq!(sum(&[one, Span::Infinite]), Span::Infinite);
        assert_eq!(sum(&[one, one]), Span::Finite(Duration::from_secs(2)));
    }
}

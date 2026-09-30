//! Reading service answers loosely: every service names things its own way,
//! so providers pick fields by JSON pointer instead of mirroring each
//! schema in types.

use serde_json::Value;

pub trait Json {
    /// The string at `pointer` (`/user/login`), or "".
    fn s(&self, pointer: &str) -> String;
    /// The non-empty string at `pointer`.
    fn opt(&self, pointer: &str) -> Option<String>;
    /// The number at `pointer` (also accepts a numeric string), or 0.
    fn n(&self, pointer: &str) -> u64;
    fn b(&self, pointer: &str) -> bool;
    /// An RFC 3339 time or epoch milliseconds at `pointer`, in seconds.
    fn time(&self, pointer: &str) -> i64;
    /// The array at `pointer`, or an empty one.
    fn list(&self, pointer: &str) -> &[Value];
}

impl Json for Value {
    fn s(&self, pointer: &str) -> String {
        self.opt(pointer).unwrap_or_default()
    }

    fn opt(&self, pointer: &str) -> Option<String> {
        match self.pointer(pointer)? {
            Value::String(s) if !s.is_empty() => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    fn n(&self, pointer: &str) -> u64 {
        match self.pointer(pointer) {
            Some(Value::Number(n)) => n.as_u64().unwrap_or_default(),
            Some(Value::String(s)) => s.parse().unwrap_or_default(),
            _ => 0,
        }
    }

    fn b(&self, pointer: &str) -> bool {
        self.pointer(pointer)
            .and_then(Value::as_bool)
            .unwrap_or_default()
    }

    fn time(&self, pointer: &str) -> i64 {
        match self.pointer(pointer) {
            Some(Value::String(s)) => parse_time(s).unwrap_or_default(),
            Some(Value::Number(n)) => n.as_i64().unwrap_or_default() / 1000,
            _ => 0,
        }
    }

    fn list(&self, pointer: &str) -> &[Value] {
        self.pointer(pointer)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }
}

/// Seconds since the Unix epoch of `2024-05-01T12:30:00Z`,
/// `2024-05-01T12:30:00.123+02:00` or `2024-05-01T12:30:00.000+0200`.
pub fn parse_time(text: &str) -> Option<i64> {
    let text = text.trim();
    let num = |range: std::ops::Range<usize>| -> Option<i64> { text.get(range)?.parse().ok() };
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !matches!(text.as_bytes().get(10), Some(b'T' | b't' | b' ')) {
        return None;
    }
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        rest = fraction.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let offset = match rest {
        "" | "Z" | "z" => 0,
        zone => {
            let sign = match zone.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let digits: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
            let hours: i64 = digits.get(0..2)?.parse().ok()?;
            let minutes: i64 = digits.get(2..4).map_or(Some(0), |m| m.parse().ok())?;
            sign * (hours * 3600 + minutes * 60)
        }
    };
    Some(days_from_civil(year, month, day) * 86400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_times() {
        assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(
            parse_time("2024-02-29T14:00:00.123+02:00"),
            Some(1_709_208_000)
        );
        assert_eq!(
            parse_time("2024-02-29T07:00:00.000-0500"),
            Some(1_709_208_000)
        );
        assert_eq!(parse_time("2024-02-29"), None);
        assert_eq!(parse_time("yesterday"), None);
    }

    #[test]
    fn reads_fields_loosely() {
        let v = json!({"a": {"b": "x", "n": "12", "t": 1_709_208_000_000_i64}, "e": ""});
        assert_eq!(v.s("/a/b"), "x");
        assert_eq!(v.opt("/e"), None);
        assert_eq!(v.n("/a/n"), 12);
        assert_eq!(v.time("/a/t"), 1_709_208_000);
        assert!(v.list("/missing").is_empty());
    }
}

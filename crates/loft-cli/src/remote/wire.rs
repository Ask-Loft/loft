//! The small pieces of HTTP and OAuth that `loft` reads and writes by hand: percent-encoding,
//! base64url, the parameters of a `WWW-Authenticate` challenge, and `Retry-After`.

use std::fmt::Write as _;

/// `text` percent-encoded but for the unreserved characters of RFC 3986.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// `pairs` as a query string or a form body, each value percent-encoded.
pub fn query(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(name, value)| format!("{name}={}", encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `text` percent-decoded, with `+` read as a space.
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = (bytes[index] == b'%' && index + 2 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[index + 1..index + 3]).ok())
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[index], hex) {
            (_, Some(byte)) => {
                out.push(byte);
                index += 2;
            }
            (b'+', None) => out.push(b' '),
            (byte, None) => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The value of `name` in a query string.
pub fn parameter(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (field, value) = pair.split_once('=').unwrap_or((pair, ""));
        (decode(field) == name).then(|| decode(value))
    })
}

/// `bytes` in base64url without padding.
pub fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (at, byte)| n | u32::from(*byte) << (16 - 8 * at));
        for at in 0..=chunk.len() {
            out.push(char::from(ALPHABET[(n >> (18 - 6 * at) & 63) as usize]));
        }
    }
    out
}

/// The parameters of a `Bearer` challenge in a `WWW-Authenticate` header, by name, or none when
/// the header holds no `Bearer` challenge.
pub fn bearer_parameters(header: &str) -> Option<Vec<(String, String)>> {
    let start = header
        .to_ascii_lowercase()
        .find("bearer")
        .filter(|at| header[at + 6..].starts_with([' ', '\t']) || header.len() == at + 6)?;
    let mut rest = header[start + 6..].trim_start();
    let mut parameters = Vec::new();
    while let Some((name, after)) = rest.split_once('=') {
        let name = name
            .trim()
            .trim_start_matches(',')
            .trim()
            .to_ascii_lowercase();
        let after = after.trim_start();
        let (value, remainder) = if let Some(rest) = after.strip_prefix('"') {
            quoted(rest)
        } else {
            let end = after.find(',').unwrap_or(after.len());
            (after[..end].trim().to_owned(), &after[end..])
        };
        parameters.push((name, value));
        rest = remainder.trim_start().trim_start_matches(',');
    }
    Some(parameters)
}

/// A quoted string's value, its escapes undone, and the text after its closing quote; `rest`
/// begins after the opening quote.
fn quoted(rest: &str) -> (String, &str) {
    let mut value = String::new();
    let mut chars = rest.char_indices();
    while let Some((at, ch)) = chars.next() {
        match ch {
            '\\' => value.extend(chars.next().map(|(_, ch)| ch)),
            '"' => return (value, &rest[at + 1..]),
            ch => value.push(ch),
        }
    }
    (value, "")
}

/// The seconds a `Retry-After` value asks for at `now`, in whole seconds since the Unix epoch:
/// a number of seconds, or the time until an HTTP date, none for a value that is neither.
pub fn retry_after(value: &str, now: u64) -> Option<u64> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds);
    }
    Some(http_date(value)?.saturating_sub(now))
}

/// An IMF-fixdate such as `Sun, 06 Nov 1994 08:49:37 GMT`, in seconds since the Unix epoch.
fn http_date(value: &str) -> Option<u64> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let parts: Vec<&str> = value.split_whitespace().collect();
    let [_, day, month, year, time, "GMT"] = parts.as_slice() else {
        return None;
    };
    let month = MONTHS.iter().position(|name| name == month)? + 1;
    let (day, year): (u64, i64) = (day.parse().ok()?, year.parse().ok()?);
    let clock: Vec<u64> = time
        .split(':')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let [hour, minute, second] = clock.as_slice() else {
        return None;
    };
    let days = days_from_civil(year, u64::try_from(month).ok()?, day)?;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

/// The days from 1970-01-01 to a date of the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: u64, day: u64) -> Option<u64> {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::try_from(month).ok()?;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5
        + i64::try_from(day).ok()?
        - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    u64::try_from(era * 146_097 + day_of_era - 719_468).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_retry_after_is_read_as_seconds_or_an_http_date() {
        assert_eq!(retry_after("7", 0), Some(7));
        let date = "Sun, 06 Nov 1994 08:49:37 GMT";
        assert_eq!(http_date(date), Some(784_111_777));
        assert_eq!(retry_after(date, 784_111_770), Some(7));
        assert_eq!(retry_after(date, 784_111_800), Some(0));
        assert_eq!(retry_after("soon", 0), None);
    }

    #[test]
    fn a_bearer_challenge_s_parameters_are_read_quoted_or_not() {
        let header = r#"Bearer resource_metadata="https://r.example/.well-known/oauth-protected-resource/x", scope="a b", error=invalid_token"#;
        let read = bearer_parameters(header).unwrap();
        assert_eq!(
            read,
            [
                (
                    "resource_metadata".to_owned(),
                    "https://r.example/.well-known/oauth-protected-resource/x".to_owned()
                ),
                ("scope".to_owned(), "a b".to_owned()),
                ("error".to_owned(), "invalid_token".to_owned()),
            ]
        );
        assert!(bearer_parameters("Basic realm=\"x\"").is_none());
    }

    #[test]
    fn a_query_round_trips_and_base64url_has_no_padding() {
        let text = query(&[("a", "b c/&="), ("d", "é")]);
        assert_eq!(parameter(&text, "a").as_deref(), Some("b c/&="));
        assert_eq!(parameter(&text, "d").as_deref(), Some("é"));
        assert_eq!(base64url(b"\xff\xfe"), "__4");
    }
}

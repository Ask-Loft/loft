//! The small pieces of HTTP and OAuth that `loft` reads and writes by hand: a query string,
//! base64url, the parameters of a `WWW-Authenticate` challenge, and `Retry-After`.

use std::str::FromStr as _;

use loftspec::publish::PublishedAt;

use crate::percent::encode;

/// `pairs` as a query string or a form body, each value percent-encoded.
pub fn query(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(name, value)| format!("{name}={}", encode(value)))
        .collect::<Vec<_>>()
        .join("&")
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
    let start = challenge_start(header, "bearer")?;
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

/// Where a challenge of `scheme` begins in `header`: at its start or after a comma, outside a
/// quoted string, followed by a space or the end.
fn challenge_start(header: &str, scheme: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;
    let mut at_start = true;
    for (at, ch) in header.char_indices() {
        if quoted {
            match (escaped, ch) {
                (false, '\\') => escaped = true,
                (false, '"') => quoted = false,
                _ => escaped = false,
            }
            continue;
        }
        if at_start && !ch.is_ascii_whitespace() {
            let rest = &header[at..];
            let named = rest
                .get(..scheme.len())
                .is_some_and(|name| name.eq_ignore_ascii_case(scheme));
            let after = rest.get(scheme.len()..).unwrap_or_default();
            if named && (after.is_empty() || after.starts_with([' ', '\t'])) {
                return Some(at);
            }
        }
        match ch {
            '"' => quoted = true,
            ',' => at_start = true,
            ch if !ch.is_ascii_whitespace() => at_start = false,
            _ => {}
        }
    }
    None
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
    let (day, year): (u8, u16) = (day.parse().ok()?, year.parse().ok()?);
    let date = format!("{year:04}-{month:02}-{day:02}T{time}Z");
    Some(PublishedAt::from_str(&date).ok()?.unix_seconds())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::percent::parameter;

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
        assert!(bearer_parameters("Basic realm=\"bearer x\"").is_none());
        let after = bearer_parameters("Basic realm=\"bearerland\", Bearer scope=\"s\"").unwrap();
        assert!(
            after.contains(&("scope".to_owned(), "s".to_owned())),
            "{after:?}"
        );
    }

    #[test]
    fn a_query_round_trips_and_base64url_has_no_padding() {
        let text = query(&[("a", "b c/&="), ("d", "é")]);
        assert_eq!(parameter(&text, "a").as_deref(), Some("b c/&="));
        assert_eq!(parameter(&text, "d").as_deref(), Some("é"));
        assert_eq!(base64url(b"\xff\xfe"), "__4");
    }
}

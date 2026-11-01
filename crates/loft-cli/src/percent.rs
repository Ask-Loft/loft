//! Percent-encoding as an address, a query string and a form body hold it (RFC 3986): the
//! encoding `loft` writes into an address, and the decoding of the queries it is sent.

use std::fmt::Write as _;

/// `text` with every byte but the unreserved ones of RFC 3986 percent-encoded.
pub fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

/// The parameters of a query string or a form body, each name and value decoded, in their
/// order.
pub fn parameters(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(name), decode(value))
        })
        .collect()
}

/// The value of the parameter `name` in a query string, decoded.
pub fn parameter(query: &str, name: &str) -> Option<String> {
    parameters(query)
        .into_iter()
        .find(|(given, _)| given == name)
        .map(|(_, value)| value)
}

/// `text` with each `%XX` turned back into its byte and each `+` into a space.
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'%' if at + 2 < bytes.len() => {
                if let (Some(high), Some(low)) = (hex(bytes[at + 1]), hex(bytes[at + 2])) {
                    decoded.push(high * 16 + low);
                    at += 3;
                    continue;
                }
                decoded.push(b'%');
            }
            b'+' => decoded.push(b' '),
            byte => decoded.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

const fn hex(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn a_percent_encoded_path_reads_back_as_written() {
        assert_eq!(decode("%2FUsers%2Fada%20b"), "/Users/ada b");
        assert_eq!(decode("feat%2Fshelf"), "feat/shelf");
        assert_eq!(decode("a+b"), "a b");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }
}

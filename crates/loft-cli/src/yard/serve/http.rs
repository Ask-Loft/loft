//! HTTP as the page server speaks it: one request read from a connection, parsed by `httparse`,
//! with its body where it names one; and one response written back. Each connection carries one
//! request and is closed after its answer, but the stream of changes, which stays open.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::io::{self, Read, Write};

use serde_json::Value;

/// The most a request's line and headers may take.
const HEAD_LIMIT: usize = 64 * 1024;
/// The most a request's body may take: a mod's batch of kept events fits well within it.
const BODY_LIMIT: usize = 8 * 1024 * 1024;
/// The most headers a request may carry.
const HEADERS: usize = 64;

/// A request as the page server reads it.
#[derive(Debug)]
pub struct Request {
    pub method: String,
    /// The path, without its query.
    pub path: String,
    /// The query's parameters, decoded, in their order.
    pub query: Vec<(String, String)>,
    /// The headers, each name in lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The value of the header `name`, given in lowercase, where the request carries it.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(given, _)| given == name)
            .map(|(_, value)| value.as_str())
    }

    /// The value of the query parameter `name`, where the request gives it.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(given, _)| given == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Reads one request from `stream`: none where the connection closed before it sent one.
///
/// # Errors
///
/// The request does not parse, or is longer than the page server takes.
pub fn read(stream: &mut impl Read) -> Result<Option<Request>, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            return if buffer.is_empty() {
                Ok(None)
            } else {
                Err("the connection closed within the request".to_owned())
            };
        }
        buffer.extend_from_slice(&chunk[..read]);
        let mut headers = [httparse::EMPTY_HEADER; HEADERS];
        let mut parsed = httparse::Request::new(&mut headers);
        match parsed.parse(&buffer).map_err(|error| error.to_string())? {
            httparse::Status::Partial if buffer.len() > HEAD_LIMIT => {
                return Err("the request's headers are too long".to_owned());
            }
            httparse::Status::Partial => {}
            httparse::Status::Complete(head) => {
                let mut request = request(&parsed)?;
                request.body = body(stream, &request, buffer[head..].to_vec())?;
                return Ok(Some(request));
            }
        }
    }
}

/// The body the request's length names: the bytes read past its head, and the rest from
/// `stream`.
fn body(stream: &mut impl Read, request: &Request, mut body: Vec<u8>) -> Result<Vec<u8>, String> {
    let length = match request.header("content-length") {
        Some(length) => length
            .trim()
            .parse::<usize>()
            .map_err(|_| "the request's length does not read".to_owned())?,
        None => 0,
    };
    if length > BODY_LIMIT {
        return Err("the request's body is too long".to_owned());
    }
    let mut chunk = [0_u8; 8192];
    while body.len() < length {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("the connection closed within the request's body".to_owned());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    Ok(body)
}

fn request(parsed: &httparse::Request<'_, '_>) -> Result<Request, String> {
    let method = parsed
        .method
        .ok_or("the request names no method")?
        .to_owned();
    let target = parsed.path.ok_or("the request names no path")?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let headers = parsed
        .headers
        .iter()
        .map(|header| {
            (
                header.name.to_ascii_lowercase(),
                String::from_utf8_lossy(header.value).into_owned(),
            )
        })
        .collect();
    Ok(Request {
        method,
        path: decode(path),
        query: parameters(query),
        headers,
        body: Vec::new(),
    })
}

fn parameters(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(name), decode(value))
        })
        .collect()
}

/// `text` with each `%XX` turned back into its byte and each `+` into a space.
fn decode(text: &str) -> String {
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

/// A response: its status, its type and its body, with any further headers.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub kind: &'static str,
    pub body: Cow<'static, [u8]>,
    pub headers: Vec<(&'static str, String)>,
}

impl Response {
    /// A JSON document with `status`.
    pub fn json(status: u16, document: &Value) -> Self {
        let mut body = serde_json::to_vec(document).expect("a JSON value serializes");
        body.push(b'\n');
        Self {
            status,
            kind: "application/json",
            body: body.into(),
            headers: Vec::new(),
        }
    }

    /// A refusal with `status`, its error of `kind` saying why.
    pub fn refused(status: u16, kind: &str, message: &str) -> Self {
        Self::json(
            status,
            &serde_json::json!({ "errors": [crate::json::error(kind, message, &[])] }),
        )
    }

    /// A file of the page, of the media type `kind`, kept in the content coding `encoding` if
    /// one is given, sent as it is built into `loft`, with its entity tag `tag`; or, where the
    /// browser holds the file of that tag already (`held`), no file and the status 304.
    pub fn file(
        kind: &'static str,
        encoding: Option<&'static str>,
        body: &'static [u8],
        tag: &str,
        held: Option<&str>,
    ) -> Self {
        let mut headers = vec![
            ("cache-control", "no-cache".to_owned()),
            ("etag", tag.to_owned()),
        ];
        if held == Some(tag) {
            return Self {
                status: 304,
                kind,
                body: Cow::Borrowed(&[]),
                headers,
            };
        }
        if let Some(encoding) = encoding {
            headers.push(("content-encoding", encoding.to_owned()));
        }
        Self {
            status: 200,
            kind,
            body: Cow::Borrowed(body),
            headers,
        }
    }
}

const fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}

/// Writes `response` to `stream`, the connection to be closed after it.
///
/// # Errors
///
/// The connection does not take the response.
pub fn write(stream: &mut impl Write, response: &Response) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\nx-content-type-options: nosniff\r\n",
        response.status,
        reason(response.status),
        response.kind,
        response.body.len()
    );
    for (name, value) in &response.headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

/// Writes the head of a stream of server-sent events to `stream`.
///
/// # Errors
///
/// The connection does not take it.
pub fn write_event_stream_head(stream: &mut impl Write) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: keep-alive\r\nx-content-type-options: nosniff\r\n\r\n",
    )?;
    stream.flush()
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

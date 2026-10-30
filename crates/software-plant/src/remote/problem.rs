//! The remote's refusals: RFC 9457 problem documents whose `code` comes from a closed set, each
//! with more members than the contract names, as a server may send; and the error answers of
//! the authorization server, which OAuth 2.0 shapes as `error` and `error_description`.

use serde_json::{Map, Value, json};

use crate::http::Response;

/// Each code of the closed set, with the status it is answered with.
pub const CODES: [(&str, u16); 13] = [
    ("invalid_request", 400),
    ("unauthenticated", 401),
    ("forbidden", 403),
    ("not_found", 404),
    ("method_not_allowed", 405),
    ("not_acceptable", 406),
    ("conflict", 409),
    ("quota_exceeded", 409),
    ("payload_too_large", 413),
    ("rate_limited", 429),
    ("internal_error", 500),
    ("corrupt_state", 500),
    ("service_unavailable", 503),
];

/// The status of a code of the closed set, or 400 for a code outside it.
#[must_use]
pub fn status_of(code: &str) -> u16 {
    CODES
        .iter()
        .find(|(known, _)| *known == code)
        .map_or(400, |(_, status)| *status)
}

/// A problem document of `code` at `status`, with `members` added to it.
#[must_use]
pub fn problem(status: u16, code: &str, members: Map<String, Value>) -> Response {
    let mut body = json!({
        "type": format!("https://remote.example/problems/{code}"),
        "title": code.replace('_', " "),
        "status": status,
        "code": code,
        "instance": "simulated",
    });
    if let Value::Object(body) = &mut body {
        body.extend(members);
    }
    let mut response = Response::json(status, &body);
    response.headers = vec![(
        "Content-Type".to_owned(),
        "application/problem+json".to_owned(),
    )];
    response
}

/// A problem document of a code of the closed set, at its own status.
#[must_use]
pub fn refusal(code: &str) -> Response {
    problem(status_of(code), code, Map::new())
}

/// `invalid_request`, naming the paths at fault.
#[must_use]
pub fn invalid(paths: &[String], detail: Option<&str>) -> Response {
    let mut members = Map::new();
    members.insert("paths".to_owned(), json!(paths));
    if let Some(detail) = detail {
        members.insert("detail".to_owned(), json!(detail));
    }
    problem(400, "invalid_request", members)
}

/// `conflict`, naming the head as its publish sequence and state root.
#[must_use]
pub fn conflict(head: Value, detail: Option<&str>) -> Response {
    let mut members = Map::new();
    members.insert("head".to_owned(), head);
    if let Some(detail) = detail {
        members.insert("detail".to_owned(), json!(detail));
    }
    problem(409, "conflict", members)
}

/// An answer that is not a problem document, for a scenario that fails without a code.
#[must_use]
pub fn plain(status: u16) -> Response {
    Response {
        status,
        headers: vec![("Content-Type".to_owned(), "text/html".to_owned())],
        body: b"<html><body>The service is not available.</body></html>".to_vec(),
    }
}

/// An OAuth 2.0 error answer: `error` at 400, as the registration and token endpoints give it.
#[must_use]
pub fn oauth(error: &str) -> Response {
    Response::json(
        400,
        &json!({ "error": error, "error_description": format!("simulated {error}") }),
    )
}

//! Calls to a remote as its contract suites make them, of the simulator and of a real remote
//! alike: a client that follows no redirect and reads every status as an answer, and each answer
//! read as its status, its headers and its body as JSON.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

/// An answer: its status, its headers by lower-case name, and its body as JSON or `null`.
pub struct Answer {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

impl Answer {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

/// A client that follows no redirect and reads every status as an answer.
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into()
}

/// `method url`, with a bearer `token` and a JSON `body` where given.
pub fn call(method: &str, url: &str, token: Option<&str>, body: Option<&Value>) -> Answer {
    let request = ureq::http::Request::builder().method(method).uri(url);
    let request = match token {
        Some(token) => request.header("Authorization", format!("Bearer {token}")),
        None => request,
    };
    let response = match body {
        Some(body) => agent().run(
            request
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .unwrap(),
        ),
        None => agent().run(request.body(()).unwrap()),
    };
    read(response.expect("the remote answers"))
}

/// `response` read as an answer.
pub fn read(mut response: ureq::http::Response<ureq::Body>) -> Answer {
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                value.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let bytes = response.body_mut().read_to_vec().unwrap_or_default();
    Answer {
        status: response.status().as_u16(),
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

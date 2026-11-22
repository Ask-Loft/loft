//! Calls to a remote as its contract suites make them, of the simulator and of a real remote
//! alike: a client that follows no redirect and reads every status as an answer, and each answer
//! read as its status, its headers and its body as JSON. A failing test shows an answer, or a
//! body, only with every token hidden, so that a run against a real remote never prints a live
//! one.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use serde_json::{Value, json};

/// What a failing test shows in place of a token or a code.
pub const REDACTED: &str = "<redacted>";

/// The members of an answer whose values are tokens, which a failing test never shows.
pub const TOKENS: [&str; 2] = ["access_token", "refresh_token"];

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

/// An answer as a failing test shows it: its status, its `Location` where it has one, and its
/// body, with every token and code hidden, and none of its other headers.
impl fmt::Debug for Answer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut shown = formatter.debug_struct("Answer");
        shown.field("status", &self.status);
        if let Some(location) = self.header("location") {
            shown.field("location", &redacted_location(location));
        }
        shown
            .field("body", &format_args!("{}", redacted(&self.body)))
            .finish_non_exhaustive()
    }
}

/// `value` as a failing test may show it: the value of every `access_token` and `refresh_token`
/// member, at any depth, replaced by [`REDACTED`].
pub fn redacted(value: &Value) -> Value {
    match value {
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(name, member)| {
                    let shown = if TOKENS.contains(&name.as_str()) {
                        json!(REDACTED)
                    } else {
                        redacted(member)
                    };
                    (name.clone(), shown)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redacted).collect()),
        other => other.clone(),
    }
}

/// `location`, the target of a redirect, with the value of each parameter of its query or its
/// fragment that holds a code or a token replaced by [`REDACTED`].
fn redacted_location(location: &str) -> String {
    let (target, fragment) = location
        .split_once('#')
        .map_or((location, None), |(target, fragment)| {
            (target, Some(fragment))
        });
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    let mut shown = path.to_owned();
    for (separator, parameters) in [('?', query), ('#', fragment)] {
        if let Some(parameters) = parameters {
            shown.push(separator);
            shown.push_str(&redacted_parameters(parameters));
        }
    }
    shown
}

/// `parameters`, joined by `&`, with the value of each that names a code or a token replaced by
/// [`REDACTED`].
fn redacted_parameters(parameters: &str) -> String {
    let hidden = |name: &str| name == "code" || TOKENS.contains(&name);
    parameters
        .split('&')
        .map(|parameter| match parameter.split_once('=') {
            Some((name, _)) if hidden(name) => format!("{name}={REDACTED}"),
            _ => parameter.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("&")
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{Answer, REDACTED, redacted};

    #[test]
    fn a_redacted_value_hides_every_token_at_any_depth() {
        let answer = json!({
            "access_token": "a-live-access-token",
            "granted": [{ "refresh_token": "a-nested-refresh-token", "scope": "notes" }],
            "refresh_token": "a-live-refresh-token",
            "resource": { "access_token": "a-nested-access-token", "kind": "notes" },
            "scope": "documents",
        });
        let shown = redacted(&answer);
        assert_eq!(
            shown,
            json!({
                "access_token": REDACTED,
                "granted": [{ "refresh_token": REDACTED, "scope": "notes" }],
                "refresh_token": REDACTED,
                "resource": { "access_token": REDACTED, "kind": "notes" },
                "scope": "documents",
            })
        );
        let unchanged = json!([{ "error": "invalid_target" }, "access_token", null]);
        assert_eq!(redacted(&unchanged), unchanged);
    }

    #[test]
    fn an_answer_shows_its_status_location_and_body_with_every_token_and_code_hidden() {
        let location = "http://127.0.0.1:4567/callback?code=a-live-code&state=the-state\
                        #access_token=a-fragment-token&token_type=Bearer";
        let answer = Answer {
            status: 302,
            headers: BTreeMap::from([("location".to_owned(), location.to_owned())]),
            body: json!({
                "access_token": "a-live-access-token",
                "refresh_token": "a-live-refresh-token",
                "scope": "documents",
            }),
        };
        assert_eq!(
            format!("{answer:?}"),
            "Answer { status: 302, location: \"http://127.0.0.1:4567/callback\
             ?code=<redacted>&state=the-state#access_token=<redacted>&token_type=Bearer\", \
             body: {\"access_token\":\"<redacted>\",\"refresh_token\":\"<redacted>\",\
             \"scope\":\"documents\"}, .. }"
        );
        let refusal = Answer {
            status: 400,
            headers: BTreeMap::new(),
            body: json!({ "error": "invalid_target" }),
        };
        assert_eq!(
            format!("{refusal:?}"),
            "Answer { status: 400, body: {\"error\":\"invalid_target\"}, .. }"
        );
    }
}

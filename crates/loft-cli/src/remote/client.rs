//! The calls to a remote: each request goes to the address as it is given followed by the
//! endpoint's path, with the bearer token in `Authorization` and nowhere else, follows no
//! redirect, and is abandoned once its time limit passes. A refusal is read by the `code` of its
//! problem document alone, and one that the remote may lift is waited out once where
//! `LOFT_REMOTE_WAIT` allows.

use std::fmt;
use std::time::Duration;

use loft_ports::clock::{Clock as _, SystemClock};
use serde_json::Value;

use super::{RemoteError, wire};
use crate::network::{self, Reply};

/// The codes a remote's problem document may hold.
const CODES: [&str; 13] = [
    "invalid_request",
    "unauthenticated",
    "forbidden",
    "not_found",
    "method_not_allowed",
    "not_acceptable",
    "conflict",
    "quota_exceeded",
    "payload_too_large",
    "rate_limited",
    "internal_error",
    "corrupt_state",
    "service_unavailable",
];

/// The codes of a refusal that the remote may lift, after which a person tries again later.
const LATER: [&str; 4] = [
    "rate_limited",
    "service_unavailable",
    "internal_error",
    "corrupt_state",
];

/// A refusal of the remote: its status, and the problem document it gave, where it gave one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub status: u16,
    /// The problem document, or `null` for an answer that is not one.
    pub problem: Value,
}

impl Refusal {
    /// The problem document's `code`, of the closed set or not.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.problem["code"].as_str()
    }

    /// The code, where it is one of the closed set: the only refusals a command acts on.
    #[must_use]
    pub fn known(&self) -> Option<&str> {
        self.code().filter(|code| CODES.contains(code))
    }

    /// Whether the remote may lift the refusal, so that a person tries again later.
    #[must_use]
    pub fn later(&self) -> bool {
        self.known().is_some_and(|code| LATER.contains(&code))
    }

    /// The problem document's `detail`, where it has one.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.problem["detail"].as_str()
    }

    fn of(reply: &Reply) -> Self {
        let problem = serde_json::from_slice::<Value>(&reply.body)
            .ok()
            .filter(|body| body["code"].is_string())
            .unwrap_or(Value::Null);
        Self {
            status: reply.status,
            problem,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "the remote answered {}", self.status)?;
        if let Some(code) = self.code() {
            write!(formatter, " with the code `{code}`")?;
        }
        if let Some(detail) = self.detail() {
            write!(formatter, ": {detail}")?;
        }
        if self.known() == Some("unauthenticated") {
            formatter.write_str(
                "; the remote refused the sign-in: `loft remote login` signs in again",
            )?;
        }
        if self.later() {
            formatter.write_str("; try again later")?;
        }
        Ok(())
    }
}

/// A remote's client: its address, the token its requests carry, their time limit, and how long
/// a run may wait out a refusal.
#[derive(Debug, Clone)]
pub struct Client {
    address: String,
    token: Option<String>,
    timeout: Duration,
    wait: Option<u64>,
}

impl Client {
    #[must_use]
    pub fn new(address: &str, token: Option<String>, timeout: Duration, wait: Option<u64>) -> Self {
        Self {
            address: address.to_owned(),
            token,
            timeout,
            wait,
        }
    }

    /// `GET <address>/<path>`, its JSON answer.
    ///
    /// # Errors
    ///
    /// The remote cannot be reached, or refuses.
    pub fn get(&self, path: &str) -> Result<Value, RemoteError> {
        self.json("GET", path, None)
    }

    /// `POST <address>/<path>` of the JSON `body`, its JSON answer.
    ///
    /// # Errors
    ///
    /// The remote cannot be reached, or refuses.
    pub fn post(&self, path: &str, body: &Value) -> Result<Value, RemoteError> {
        self.json("POST", path, Some(&body.to_string().into_bytes()))
    }

    /// `method <address>/<path>` of `body`, made once more after a refusal that the remote may
    /// lift where `LOFT_REMOTE_WAIT` allows the wait it asks for: its JSON answer.
    fn json(&self, method: &str, path: &str, body: Option<&[u8]>) -> Result<Value, RemoteError> {
        let reply = self.call(method, path, body)?;
        let reply = match self.waited(&reply) {
            Some(length) => {
                SystemClock::from_env().wait(length);
                self.call(method, path, body)?
            }
            None => reply,
        };
        let read = serde_json::from_slice::<Value>(&reply.body).ok();
        match read {
            Some(value) if (200..300).contains(&reply.status) && value.is_object() => Ok(value),
            _ => Err(RemoteError::Refused {
                address: self.address.clone(),
                refusal: Refusal::of(&reply),
            }),
        }
    }

    /// How long to wait before the call is made once more, for a refusal that the remote may
    /// lift whose `Retry-After` asks for no more than `LOFT_REMOTE_WAIT` allows.
    fn waited(&self, reply: &Reply) -> Option<Duration> {
        let allowed = self.wait?;
        if reply.status < 400 || !Refusal::of(reply).later() {
            return None;
        }
        let asked = wire::retry_after(reply.retry_after.as_deref()?, super::now()?)?;
        (asked <= allowed).then(|| Duration::from_secs(asked))
    }

    /// `method <address>/<path>`, with the token and the JSON `body` where given.
    ///
    /// # Errors
    ///
    /// The remote cannot be reached, or gives no answer within the time limit.
    pub fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Reply, RemoteError> {
        let url = format!("{}/{path}", self.address);
        network::allowed(&url).ok_or_else(|| RemoteError::Address(url.clone()))?;
        let bearer = self.token.as_ref().map(|token| format!("Bearer {token}"));
        let mut headers = vec![("Accept", "application/json")];
        if let Some(bearer) = &bearer {
            headers.push(("Authorization", bearer));
        }
        if body.is_some() {
            headers.push(("Content-Type", "application/json"));
        }
        network::call(method, &url, &headers, body, self.timeout).map_err(|reason| {
            RemoteError::Unreachable {
                address: self.address.clone(),
                reason,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn refusal(status: u16, body: &Value) -> Refusal {
        Refusal::of(&Reply {
            status,
            retry_after: None,
            www_authenticate: Vec::new(),
            body: body.to_string().into_bytes(),
        })
    }

    #[test]
    fn a_refusal_is_read_by_its_code_alone() {
        let known = refusal(
            409,
            &json!({ "type": "x", "title": "y", "code": "conflict", "extra": 1 }),
        );
        assert_eq!(known.known(), Some("conflict"));
        let outside = refusal(418, &json!({ "code": "teapot" }));
        assert_eq!((outside.known(), outside.code()), (None, Some("teapot")));
        assert!(outside.to_string().contains("418") && outside.to_string().contains("teapot"));
        let plain = refusal(502, &json!("<html>"));
        assert_eq!((plain.known(), plain.code()), (None, None));
        assert!(plain.to_string().contains("502"));
    }

    #[test]
    fn only_the_codes_the_remote_may_lift_say_to_try_again_later() {
        for code in LATER {
            assert!(
                refusal(503, &json!({ "code": code }))
                    .to_string()
                    .contains("try again later")
            );
        }
        for code in ["conflict", "forbidden", "teapot"] {
            assert!(
                !refusal(409, &json!({ "code": code }))
                    .to_string()
                    .contains("try again later")
            );
        }
        assert!(
            refusal(401, &json!({ "code": "unauthenticated" }))
                .to_string()
                .contains("loft remote login")
        );
    }
}

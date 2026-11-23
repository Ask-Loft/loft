//! The calls to a remote: each request goes to the address as it is given followed by the
//! endpoint's path, with the bearer token in `Authorization` and nowhere else, follows no
//! redirect, and is abandoned once its time limit passes. A refusal is read by the `code` of its
//! problem document alone, and one that the remote may lift is waited out once where
//! `LOFT_REMOTE_WAIT` allows.

use std::cell::RefCell;
use std::fmt;
use std::time::Duration;

use loft_ports::clock::{Clock as _, SystemClock};
use serde_json::Value;

use super::{RemoteError, Whose, signin, signins, wire};
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

    /// The refusal in words, where a refused sign-in says that `login`, a command, signs in
    /// again, where one can.
    #[must_use]
    pub fn said(&self, login: Option<&str>) -> String {
        let mut said = String::new();
        self.write(&mut said, login)
            .expect("a String takes every write");
        said
    }

    /// What the remote answered: its status, and its code and detail where it gave them.
    #[must_use]
    pub fn answered(&self) -> String {
        let code =
            (self.code()).map_or_else(String::new, |code| format!(" with the code `{code}`"));
        let detail = (self.detail()).map_or_else(String::new, |detail| format!(": {detail}"));
        format!("the remote answered {}{code}{detail}", self.status)
    }

    fn write(&self, out: &mut impl fmt::Write, login: Option<&str>) -> fmt::Result {
        out.write_str(&self.answered())?;
        if self.known() == Some("unauthenticated") {
            out.write_str("; the remote refused the sign-in")?;
            if let Some(login) = login {
                write!(out, ": {login} signs in again")?;
            }
        }
        if self.later() {
            out.write_str("; try again later")?;
        }
        Ok(())
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(formatter, Some("`loft remote login`"))
    }
}

/// A remote's client: its address, the token its requests carry, their time limit, how long a
/// run may wait out a refusal, and, for another document's remote that a command may sign in to,
/// how it signs in afresh where the remote refuses its sign-in.
#[derive(Debug, Clone)]
pub struct Client {
    address: String,
    token: RefCell<Option<String>>,
    timeout: Duration,
    wait: Option<u64>,
    /// What the client does where the remote answers `401`; nothing more where it holds none.
    unauthorized: RefCell<Option<Unauthorized>>,
}

/// What a client does where the remote answers `401`.
#[derive(Debug, Clone)]
enum Unauthorized {
    /// Signs the person in afresh, once, and asks again.
    Renew(Renewal),
    /// Answers with the remote's refusal beside the error for the person's missing sign-in, as
    /// a client that sends no token for want of one does.
    Missing(Missing),
}

/// The sign-in a client sends no token for want of: the key of the document it reads, and the
/// error for that sign-in.
#[derive(Debug, Clone)]
struct Missing {
    key: String,
    error: RemoteError,
}

/// How a client signs in afresh, once: the handle under which `documents` lists another
/// document, that document's key where it lists it, and whether a browser may be opened.
#[derive(Debug, Clone)]
struct Renewal {
    handle: String,
    key: Option<String>,
    browser: bool,
}

impl Client {
    #[must_use]
    pub fn new(address: &str, token: Option<String>, timeout: Duration, wait: Option<u64>) -> Self {
        Self {
            address: address.to_owned(),
            token: RefCell::new(token),
            timeout,
            wait,
            unauthorized: RefCell::new(None),
        }
    }

    /// The client of the document `key`, sending no token for want of a sign-in, whose remote's
    /// `401` is answered with the refusal beside `missing`, the error for that missing sign-in.
    #[must_use]
    pub fn unsigned(self, key: &str, missing: RemoteError) -> Self {
        *self.unauthorized.borrow_mut() = Some(Unauthorized::Missing(Missing {
            key: key.to_owned(),
            error: missing,
        }));
        self
    }

    /// The client, signing the person in afresh to the remote of `whose`, once, where the remote
    /// refuses the sign-in its token carries; a browser is opened where `browser` allows. Only
    /// another document's remote is signed in to so.
    #[must_use]
    pub fn renewing(self, whose: Whose<'_>, browser: bool) -> Self {
        if let Whose::Other { handle, key } = whose {
            *self.unauthorized.borrow_mut() = Some(Unauthorized::Renew(Renewal {
                handle: handle.to_owned(),
                key: key.map(str::to_owned),
                browser,
            }));
        }
        self
    }

    /// The document's address.
    pub fn address(&self) -> &str {
        &self.address
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
        let reply = self.answered(method, path, body)?;
        let reply = if self.renewed(&reply)? {
            self.answered(method, path, body)?
        } else {
            reply
        };
        if reply.status == 401
            && let Some(Unauthorized::Missing(missing)) = &*self.unauthorized.borrow()
        {
            return Err(RemoteError::Unsigned {
                key: missing.key.clone(),
                address: self.address.clone(),
                refusal: Refusal::of(&reply),
                missing: Box::new(missing.error.clone()),
            });
        }
        let read = serde_json::from_slice::<Value>(&reply.body).ok();
        match read {
            Some(value) if (200..300).contains(&reply.status) && value.is_object() => Ok(value),
            _ => Err(RemoteError::Refused {
                address: self.address.clone(),
                refusal: Refusal::of(&reply),
            }),
        }
    }

    /// `method <address>/<path>` of `body`, made once more after a refusal that the remote may
    /// lift where `LOFT_REMOTE_WAIT` allows the wait it asks for.
    fn answered(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Reply, RemoteError> {
        let reply = self.call(method, path, body)?;
        match self.waited(&reply) {
            Some(length) => {
                SystemClock::from_env().wait(length);
                self.call(method, path, body)
            }
            None => Ok(reply),
        }
    }

    /// Whether the person was signed in afresh, and the new sign-in kept, because `reply`
    /// refused the sign-in of a client that may make one; each client does so once.
    fn renewed(&self, reply: &Reply) -> Result<bool, RemoteError> {
        if reply.status != 401 || Refusal::of(reply).known() != Some("unauthenticated") {
            return Ok(false);
        }
        let taken = self.unauthorized.borrow_mut().take();
        let renewal = match taken {
            Some(Unauthorized::Renew(renewal)) => renewal,
            kept => {
                *self.unauthorized.borrow_mut() = kept;
                return Ok(false);
            }
        };
        let whose = Whose::Other {
            handle: &renewal.handle,
            key: renewal.key.as_deref(),
        };
        let sign_in = signin::sign_in(&self.address, whose, renewal.browser, self.timeout)?;
        signins::save(&self.address, &sign_in)?;
        *self.token.borrow_mut() = Some(sign_in.access_token);
        Ok(true)
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
        super::contactable(&url)?;
        let bearer = (self.token.borrow().as_ref()).map(|token| format!("Bearer {token}"));
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

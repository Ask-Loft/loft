//! Who the page server answers. Every request names as its `Host` `localhost` or the address of
//! the machine it reached, so that a web page under another name that resolves to this machine
//! reaches nothing. A request that changes something, Yard Log, the trees served or a session's
//! events, comes either from the page the page server served, whose `Origin` is its own at the
//! address the request reached, or from the mod, which sends no `Origin` and carries the control
//! token instead.

use super::http::{Request, Response};
use super::listener::Reached;

/// What a request does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// It only reads.
    Read,
    /// It changes Yard Log, the trees served or a session's events.
    Change,
}

/// Whether `request` may be answered, or the refusal that answers it instead.
pub fn admit(
    request: &Request,
    access: Access,
    reached: Reached,
    token: &str,
) -> Result<(), Response> {
    let host = request.header("host").unwrap_or_default();
    if !reached.is_named_by(host) {
        return Err(Response::refused(
            403,
            "host",
            "the request is not addressed to the page server",
        ));
    }
    if access == Access::Read {
        return Ok(());
    }
    if let Some(origin) = request.header("origin") {
        let from_page = origin
            .get(.."http://".len())
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
            && reached.is_named_by(&origin["http://".len()..]);
        return if from_page {
            Ok(())
        } else {
            Err(Response::refused(
                403,
                "origin",
                "the request comes from a page the page server did not serve",
            ))
        };
    }
    let given = request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if same(given.trim().as_bytes(), token.as_bytes()) {
        Ok(())
    } else {
        Err(Response::refused(
            401,
            "token",
            "the request carries neither the page's origin nor the control token",
        ))
    }
}

/// Whether two byte strings are equal, compared in a time that does not depend on where they
/// first differ.
fn same(given: &[u8], token: &[u8]) -> bool {
    if given.len() != token.len() || token.is_empty() {
        return false;
    }
    given
        .iter()
        .zip(token)
        .fold(0_u8, |differ, (a, b)| differ | (a ^ b))
        == 0
}

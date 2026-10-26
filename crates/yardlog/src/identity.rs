//! Host-qualified session identity, with the legacy Claude namespace kept intact.

use crate::act::CLAUDE_CODE;

/// Whether a host can name an adapter and its session-file namespace.
#[must_use]
pub fn is_host_id(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 64
        && host.as_bytes()[0].is_ascii_lowercase()
        && host
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// The storage identity of a host's session. The original session stays in each record.
#[must_use]
pub fn session_key(host: &str, session: &str) -> String {
    if host == CLAUDE_CODE {
        session.to_owned()
    } else {
        format!("{host}~{session}")
    }
}

//! Host-qualified session identity, with the legacy Claude namespace kept intact.

use crate::act::CLAUDE_CODE;

/// Whether a session or subagent id can safely name its machine-local records.
#[must_use]
pub fn is_session_id(session: &str) -> bool {
    !session.is_empty()
        && session.len() <= 128
        && !session.starts_with('.')
        && session
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

/// Reads an explicitly identified task assignee, never a person or a display name.
///
/// # Errors
/// An identity is incomplete, invalid, or contains an unknown field.
pub fn agent(value: &serde_json::Value) -> Result<crate::Agent, String> {
    let map = value.as_object().ok_or("assignee is an agent mapping")?;
    if map
        .keys()
        .any(|key| !["host", "session", "agent", "agentType"].contains(&key.as_str()))
    {
        return Err("assignee holds an unknown field".to_owned());
    }
    let host = value["host"]
        .as_str()
        .filter(|host| is_host_id(host))
        .ok_or("assignee needs a valid host")?;
    let session = value["session"]
        .as_str()
        .filter(|session| is_session_id(session))
        .ok_or("assignee needs a valid session")?;
    match (map.get("agent"), map.get("agentType")) {
        (None, None) => Ok(crate::Agent::main_in(host, session)),
        (Some(id), Some(kind)) => {
            let id = id
                .as_str()
                .filter(|id| is_session_id(id))
                .ok_or("assignee needs a valid agent id")?;
            let kind = kind
                .as_str()
                .filter(|kind| !kind.trim().is_empty())
                .ok_or("assignee needs an agent type")?;
            Ok(crate::Agent::sub_in(host, session, id, kind))
        }
        _ => Err("assignee needs both agent and agentType for a subagent".to_owned()),
    }
}

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

//! When each session was last heard, in `~/.yard/sessions.json`: a session is heard each time
//! its mod runs `loft yard serve`, and each time it runs a `loft yard` command that names it. A
//! session not heard for 60 seconds has stopped, and the submissions it sent may be taken up.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{Map, Value, json};
use yardlog::Time;
use yardlog_store::home::{Home, HomeFile};

use super::failure::Failure;

/// How long a session goes unheard before it counts as stopped.
pub const STOPPED_AFTER: Duration = Duration::from_secs(60);

/// Records that `session` was heard at `at`.
pub fn heard(home: &Home, session: &str, at: Time) -> Result<(), Failure> {
    let at = json!(at.to_string());
    home.update(HomeFile::Sessions, |bytes| {
        let mut map = object(bytes.as_deref());
        if map.get(session) == Some(&at) {
            return None;
        }
        map.insert(session.to_owned(), at);
        let ordered: BTreeMap<&String, &Value> = map.iter().collect();
        let mut text = serde_json::to_string_pretty(&ordered).expect("a JSON value serializes");
        text.push('\n');
        Some(text.into_bytes())
    })?;
    Ok(())
}

/// When `session` was last heard, where it ever was.
pub fn last_heard(home: &Home, session: &str) -> Result<Option<Time>, Failure> {
    let map = object(home.read(HomeFile::Sessions)?.as_deref());
    Ok(map
        .get(session)
        .and_then(Value::as_str)
        .and_then(|text| text.parse().ok()))
}

/// Whether `session`, last heard at `last`, has stopped by `now`: it was never heard, or not
/// for [`STOPPED_AFTER`].
pub fn stopped(last: Option<Time>, now: Time) -> bool {
    last.is_none_or(|last| {
        now.unix_seconds().saturating_sub(last.unix_seconds()) >= STOPPED_AFTER.as_secs()
    })
}

fn object(bytes: Option<&[u8]>) -> Map<String, Value> {
    match bytes.map(serde_json::from_slice::<Value>) {
        Some(Ok(Value::Object(map))) => map,
        _ => Map::new(),
    }
}

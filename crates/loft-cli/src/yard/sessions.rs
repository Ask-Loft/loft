//! When each session was last heard, in `~/.yard/sessions.json`: a session is heard each time
//! the page server hears from its mod, on the mod's beat or with an event it posts, and when its
//! mod runs `loft yard serve`; no other command records it. A session not heard for 60 seconds
//! has stopped, and the submissions it sent may be taken up.

use std::time::Duration;

use serde_json::{Value, json};
use yardlog::Time;
use yardlog_store::home::{Home, HomeFile};

use super::failure::Failure;
use super::home_json;

/// How long a session goes unheard before it counts as stopped.
pub const STOPPED_AFTER: Duration = Duration::from_secs(60);

/// Records that `session` was heard at `at`.
pub fn heard(home: &Home, session: &str, at: Time) -> Result<(), Failure> {
    home_json::set(home, HomeFile::Sessions, session, json!(at.to_string()))?;
    Ok(())
}

/// When `session` was last heard, where it ever was.
pub fn last_heard(home: &Home, session: &str) -> Result<Option<Time>, Failure> {
    let map = home_json::object(home.read(HomeFile::Sessions)?.as_deref());
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

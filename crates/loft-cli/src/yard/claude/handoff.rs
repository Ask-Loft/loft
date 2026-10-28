//! The continuation of a session after `/clear` or a resume. Claude Code gives the session start
//! that follows only the new session's id, but runs the ended session's end just before it in the
//! same process, so the end leaves the ended session as a hand-off for that process and the
//! start takes it, records the continuation and is briefed as the session it continues.

use serde_json::json;
use yardlog::Time;
use yardlog_store::home::Home;

use super::hook::Hook;
use super::store::{HANDOFFS, Store};
use crate::yard::caller::record_continuation;

/// How long a hand-off waits for the session start that takes it, in seconds.
const FRESH: u64 = 60;

/// The end reasons and start sources of a session that another continues.
const CONTINUED: [&str; 2] = ["clear", "resume"];

/// Leaves the session a `SessionEnd` hook ends as a hand-off for the Claude Code process `pid`,
/// where it ends by `/clear` or a resume.
pub fn leave(hook: &Hook, pid: u32, at: Time) -> Result<(), String> {
    if !hook
        .field("reason")
        .is_some_and(|reason| CONTINUED.contains(&reason))
    {
        return Ok(());
    }
    let Some(store) = Store::of_home() else {
        return Ok(());
    };
    store.update(HANDOFFS, |handoffs| {
        handoffs.insert(
            pid.to_string(),
            json!({"session": hook.session(), "at": at.to_string()}),
        );
    })
}

/// Takes the hand-off the Claude Code process `pid` left, where the `SessionStart` hook starts by
/// `/clear` or a resume and the hand-off is fresh, and records that the hook's session continues
/// the session it names; whether it did.
pub fn take(hook: &Hook, pid: u32, at: Time) -> Result<bool, String> {
    if !hook
        .field("source")
        .is_some_and(|source| CONTINUED.contains(&source))
    {
        return Ok(false);
    }
    let (Some(store), Some(home)) = (Store::of_home(), Home::from_env()) else {
        return Ok(false);
    };
    let taken = store.update(HANDOFFS, |handoffs| handoffs.remove(&pid.to_string()))?;
    let Some(ended) = taken.and_then(|taken| {
        let left = taken["at"].as_str()?.parse::<Time>().ok()?;
        let fresh = at.unix_seconds().saturating_sub(left.unix_seconds()) <= FRESH;
        let ended = taken["session"].as_str()?.to_owned();
        (fresh && ended != hook.session()).then_some(ended)
    }) else {
        return Ok(false);
    };
    record_continuation(&home, yardlog::act::CLAUDE_CODE, hook.session(), &ended)
        .map_err(super::super::codex::message)?;
    Ok(true)
}

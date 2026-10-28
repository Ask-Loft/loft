//! The main agent's plan mode, heard from the permission mode Claude Code gives each
//! `UserPromptSubmit`, `PostToolUse` and `Stop` hook: each session's last mode is kept in
//! `~/.yard/claude/modes.json`, and a change to or from `plan` is an event. A session first heard
//! outside plan mode says nothing, and a subagent's own mode is not the session's.

use super::hook::Hook;
use super::store::{MODES, Store};

/// The events that carry the permission mode.
pub const CARRY: [&str; 3] = ["UserPromptSubmit", "PostToolUse", "Stop"];

/// The event a hook's permission mode makes, `plan.start` or `plan.end`, where it changes the
/// session's plan mode; nothing for a subagent's hook or one that names no mode.
pub fn heard(hook: &Hook) -> Result<Option<&'static str>, String> {
    let Some(mode) = hook.field("permission_mode") else {
        return Ok(None);
    };
    if hook.is_subagent() {
        return Ok(None);
    }
    let Some(store) = Store::of_home() else {
        return Ok(None);
    };
    let planning = mode == "plan";
    store.update(MODES, |modes| {
        let was = modes.insert(hook.session().to_owned(), planning.into());
        let was = was.and_then(|was| was.as_bool());
        match was {
            Some(was) if was == planning => None,
            None if !planning => None,
            _ => Some(if planning { "plan.start" } else { "plan.end" }),
        }
    })
}

/// Forgets the session's last mode as it ends; writes nothing where no mode is kept.
pub fn forget(hook: &Hook) -> Result<(), String> {
    let Some(store) = Store::of_home().filter(|store| store.holds(MODES)) else {
        return Ok(());
    };
    store.update(MODES, |modes| {
        modes.remove(hook.session());
    })
}

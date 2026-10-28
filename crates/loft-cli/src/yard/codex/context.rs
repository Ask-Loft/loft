//! The context the hook gives through native additionalContext: as a session or subagent
//! starts, the caller's briefing, which its read records as the caller's last, followed only by
//! the caller's arguments and whether decision notifications reach it; with each later prompt,
//! only whether they do.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::Result;
use super::commands::Caller;
use super::translate::Hook;

fn clipped(text: &str, maximum: usize) -> String {
    if text.chars().count() <= maximum {
        return text.to_owned();
    }
    format!("{}…", text.chars().take(maximum - 1).collect::<String>())
}

/// The context as the caller starts, from the read of its `briefing`.
pub fn start(hook: &Hook, caller: &Caller, briefing: &Result<Value>) -> Value {
    let mut text = match briefing {
        Ok(doc) => doc["context"]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        Err(error) => format!("Yard is unavailable: {}", clipped(error, 400)),
    };
    let arguments: Vec<String> = caller
        .args()
        .into_iter()
        .map(|argument| clipped(&argument, 128))
        .collect();
    let _ = writeln!(
        text,
        "\nYard caller: {}. Use these arguments for Yard commands.",
        arguments.join(" ")
    );
    text.push_str(notifications(hook, caller));
    output(hook, &text)
}

/// The context with a later prompt: whether decision notifications reach the caller.
pub fn prompt(hook: &Hook, caller: &Caller) -> Value {
    output(hook, notifications(hook, caller))
}

fn output(hook: &Hook, text: &str) -> Value {
    json!({"hookSpecificOutput":{"hookEventName":hook.name(),"additionalContext":text}})
}

fn notifications(hook: &Hook, caller: &Caller) -> &'static str {
    let delivery = super::state::Files::of(hook)
        .and_then(|files| files.read())
        .map(|s| s.notifications.availability(caller.thread()))
        .unwrap_or_default();
    match delivery {
        super::notification_state::Availability::Available => {
            "Yard decision notifications are available; after submitting, you may end the turn while the worker monitors the decision."
        }
        super::notification_state::Availability::Uncertain => {
            "Yard decision notification delivery is uncertain; unread work is retried after receipt checks, with an exceptional duplicate possible."
        }
        _ => {
            "Yard decision notifications are unavailable; use explicit Yard waiting while a decision is needed."
        }
    }
}

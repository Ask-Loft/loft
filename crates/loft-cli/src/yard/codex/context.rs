//! Small session context, from an unmarked JSON read.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::commands::{self, Caller};
use super::translate::Hook;

pub fn output(hook: &Hook, caller: &Caller, port: u16) -> Value {
    let mut text = format!(
        "Yard caller: {}. Use these arguments for Yard commands.\n",
        caller.args().join(" ")
    );
    text.push_str(notification_context(hook, caller));
    match commands::read(hook.dir(), caller, port) {
        Ok(doc) => describe(&mut text, &doc, caller, port),
        Err(error) => {
            let _ = writeln!(text, "Yard is unavailable: {error}");
        }
    }
    text.push_str("Read with --mark-read before writing. Questions and plan approvals use the Yard page; native Codex mode and permissions still apply. Native hooks carry no usage counters. ");
    let available = super::state::Files::of(hook)
        .and_then(|files| files.read())
        .is_ok_and(|state| {
            state
                .usage
                .active
                .values()
                .any(|active| active.source == super::usage::Source::Available)
        });
    text.push_str(if available { "App-server usage is available for an observed native turn. " } else { "App-server usage is currently unavailable; observation is attempted during identified native turns. " });
    text.push_str(
        "Missing sources and observation gaps stay unknown. Load the yard skill for the workflow.",
    );
    let count = text.chars().count();
    if count > 2000 {
        text = text.chars().take(1940).collect();
        let _ = write!(text, "\n[{} characters omitted]", count - 1940);
    }
    json!({"hookSpecificOutput":{"hookEventName":hook.name(),"additionalContext":text}})
}

fn notification_context(hook: &Hook, caller: &Caller) -> &'static str {
    let thread = caller
        .agent
        .as_ref()
        .map_or(caller.session.as_str(), |(id, _)| id.as_str());
    let delivery = super::state::Files::of(hook)
        .and_then(|files| files.read())
        .map(|s| s.notifications.availability(thread))
        .unwrap_or_default();
    match delivery {
        super::notification_state::Availability::Available => {
            "Yard decision notifications are available; after submitting, you may end the turn while the worker monitors the decision. "
        }
        super::notification_state::Availability::Uncertain => {
            "Yard decision notification delivery is uncertain; unread work is retried after receipt checks, with an exceptional duplicate possible. "
        }
        _ => {
            "Yard decision notifications are unavailable; use explicit Yard waiting while a decision is needed. "
        }
    }
}

fn describe(text: &mut String, doc: &Value, caller: &Caller, port: u16) {
    let page = doc["page"]["url"]
        .as_str()
        .map_or_else(|| format!("http://127.0.0.1:{port}/"), str::to_owned);
    let _ = writeln!(text, "Page: {page}");
    let _ = writeln!(
        text,
        "Revision: {}; last read: {}.",
        doc["revision"],
        doc.get("lastRead").unwrap_or(&Value::Null)
    );
    if let Some(stage) = commands::active(doc, caller) {
        let _ = writeln!(
            text,
            "Active stage: {} ({})",
            stage["id"], stage["version"]["body"]["title"]
        );
    }
    let groups = doc["submissions"].as_array().map_or(&[][..], Vec::as_slice);
    let pending: Vec<_> = groups
        .iter()
        .filter(|group| caller.owns(&group["agent"]))
        .flat_map(|group| group["open"].as_array().into_iter().flatten())
        .filter_map(|submission| submission["key"].as_str())
        .collect();
    let _ = writeln!(text, "Pending submissions: {}.", pending.join(", "));
    if doc["revision"] == 0 {
        text.push_str("The branch has no Yard Log yet.\n");
    }
    if doc["yardLog"]["closedBy"]
        .as_str()
        .is_some_and(|by| by != "open")
    {
        text.push_str("Yard Log is closed.\n");
    }
    if doc["ignored"] == true {
        text.push_str("Git ignores .yard/; leave Yard Log out of commits.\n");
    }
}

//! Small session context, from an unmarked JSON read.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::commands::{self, Caller};
use super::task_writes;
use super::translate::Hook;

fn clipped(text: &str, maximum: usize) -> String {
    if text.chars().count() <= maximum {
        return text.to_owned();
    }
    format!("{}…", text.chars().take(maximum - 1).collect::<String>())
}

pub fn output(hook: &Hook, caller: &Caller, port: u16) -> Value {
    let mut text = format!(
        "Yard caller: {}. Use these arguments for Yard commands.\n",
        caller
            .args()
            .into_iter()
            .map(|argument| clipped(&argument, 128))
            .collect::<Vec<_>>()
            .join(" ")
    );
    text.push_str(notification_context(hook, caller));
    text.push_str("Zero cache writes from ChatGPT or unclassified sources are unavailable. Verified API-key zeros and positive reported counts remain measured. Raw source values are preserved. ");
    match commands::read(hook.dir(), caller, port) {
        Ok(doc) => describe(&mut text, &doc, caller, port),
        Err(error) => {
            let _ = writeln!(text, "Yard is unavailable: {error}");
        }
    }
    let tail = footer(hook);
    let room = 2000 - tail.chars().count();
    let count = text.chars().count();
    if count > room {
        text = text.chars().take(room - 60).collect();
        let _ = write!(text, "\n[{} characters omitted]", count - (room - 60));
    }
    text.push_str(&tail);
    json!({"hookSpecificOutput":{"hookEventName":hook.name(),"additionalContext":text}})
}

fn footer(hook: &Hook) -> String {
    let mut tail = String::new();
    tail.push_str("Read with --mark-read before writing. Questions and plan approvals use the Yard page; native Codex mode and permissions still apply. Native hooks carry no usage counters. ");
    let available = super::state::Files::of(hook)
        .and_then(|files| files.read())
        .is_ok_and(|state| {
            state
                .usage
                .active
                .values()
                .any(|active| active.source == super::usage::Source::Available)
        });
    tail.push_str(if available { "App-server usage is available for an observed native turn. " } else { "App-server usage is currently unavailable; observation is attempted during identified native turns. " });
    tail.push_str(
        "Missing sources and observation gaps stay unknown. Load the yard skill for the workflow.",
    );
    tail
}

fn notification_context(hook: &Hook, caller: &Caller) -> &'static str {
    let thread = caller.thread();
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
    let _ = writeln!(text, "Page: {}", clipped(&page, 350));
    let _ = writeln!(
        text,
        "Revision: {}; last read: {}.",
        doc["revision"],
        doc.get("lastRead").unwrap_or(&Value::Null)
    );
    assignments(text, doc, caller);
    if let Some(stage) = commands::active(doc, caller) {
        let _ = writeln!(
            text,
            "Active stage: {} ({})",
            clipped(&stage["id"].to_string(), 100),
            clipped(&stage["version"]["body"]["title"].to_string(), 100)
        );
    }
    let groups = doc["submissions"].as_array().map_or(&[][..], Vec::as_slice);
    let pending: Vec<_> = groups
        .iter()
        .filter(|group| caller.owns(&group["agent"]))
        .flat_map(|group| group["open"].as_array().into_iter().flatten())
        .filter_map(|submission| submission["key"].as_str())
        .collect();
    let _ = writeln!(
        text,
        "Pending submissions: {}.",
        clipped(&pending.join(", "), 160)
    );
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

fn assignments(text: &mut String, doc: &Value, caller: &Caller) {
    let assigned: Vec<_> = task_writes::assigned(doc, caller)
        .filter(|(_, block)| {
            !matches!(
                block["version"]["body"]["state"].as_str(),
                Some("done" | "dropped")
            )
        })
        .collect();
    if assigned.is_empty() {
        return;
    }
    let _ = writeln!(
        text,
        "Tasks assigned to you: {}. Read Yard Log for details.",
        assigned.len()
    );
    for (index, (stage, block)) in assigned.iter().enumerate() {
        let line = format!("Task {} in stage {}.\n", block["id"], stage["id"]);
        if text.chars().count() + line.chars().count() > 1250 {
            let _ = writeln!(
                text,
                "{} more assigned tasks; read Yard Log.",
                assigned.len() - index
            );
            break;
        }
        text.push_str(&line);
    }
}

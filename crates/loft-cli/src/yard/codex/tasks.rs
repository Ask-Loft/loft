//! Successful native plan updates become tasks, with stable text-and-occurrence identities.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::Result;
use super::commands::{self, Caller};
use super::task_bindings::{self, Reference};
use super::task_writes;
use super::translate::Hook;

fn successful(response: &Value) -> bool {
    response
        .as_str()
        .is_some_and(|text| text.trim() == "Plan updated")
        || response["success"] == true
        || (response["isError"] == false && response["content"].is_array())
}

fn steps(hook: &Hook) -> Option<Vec<(&str, &str)>> {
    if hook.name() != "PostToolUse"
        || hook.0["tool_name"] != "update_plan"
        || !successful(&hook.0["tool_response"])
    {
        return None;
    }
    let steps: Vec<_> = hook.0["tool_input"]["plan"]
        .as_array()?
        .iter()
        .map(|step| {
            let text = step["step"]
                .as_str()
                .filter(|text| !text.trim().is_empty())?;
            let status = match step["status"].as_str()? {
                "pending" => "open",
                "in_progress" => "active",
                "completed" => "done",
                _ => return None,
            };
            Some((text, status))
        })
        .collect::<Option<_>>()?;
    let mut seen = BTreeSet::new();
    for (text, _) in &steps {
        match task_bindings::reference(text) {
            Reference::Invalid => return None,
            Reference::Bound(id) if !seen.insert(id) => return None,
            _ => {}
        }
    }
    Some(steps)
}

fn digest(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))[..24].to_owned()
}

fn batch(doc: &Value, caller: &Caller, steps: &[(&str, &str)]) -> Vec<Value> {
    let mut batch = task_bindings::batch(doc, caller, steps);
    let Some(stage) = commands::active(doc, caller) else {
        return batch;
    };
    let prefix = format!(
        "codex-task-{}-",
        digest(&json!([caller.session, caller.agent, stage["key"]]))
    );
    let existing: BTreeMap<_, _> = stage["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| Some((block["id"].as_str()?, block)))
        .filter(|(id, block)| id.starts_with(&prefix) && block["kind"] == "task")
        .collect();
    let mut occurrences = BTreeMap::new();
    let mut seen: BTreeSet<String> = steps
        .iter()
        .filter_map(|(text, _)| match task_bindings::reference(text) {
            Reference::Bound(id) => Some(id.to_owned()),
            _ => None,
        })
        .collect();
    for (text, status) in steps {
        if task_bindings::reference(text) != Reference::Plain {
            continue;
        }
        let occurrence = occurrences.entry(*text).or_insert(0usize);
        let id = format!("{prefix}{}", digest(&json!([text, occurrence])));
        *occurrence += 1;
        if seen.contains(&id) {
            continue;
        }
        seen.insert(id.clone());
        let body = existing
            .get(id.as_str())
            .map(|block| &block["version"]["body"]);
        if body.is_some_and(|body| {
            !task_writes::executable(body, caller)
                || (body["assignee"].is_object()
                    && matches!(body["state"].as_str(), Some("done" | "dropped")))
        }) {
            continue;
        }
        let mut block = body.cloned().unwrap_or_else(|| json!({"title":text}));
        block["state"] = json!(status);
        if body == Some(&block) {
            continue;
        }
        block["id"] = json!(id);
        block["kind"] = json!("task");
        block["stage"] = stage["id"].clone();
        batch.push(block);
    }
    for (id, held) in existing {
        let body = &held["version"]["body"];
        if seen.contains(id)
            || !task_writes::executable(body, caller)
            || matches!(body["state"].as_str(), Some("done" | "dropped"))
        {
            continue;
        }
        let mut block = body.clone();
        block["id"] = json!(id);
        block["kind"] = json!("task");
        block["stage"] = stage["id"].clone();
        block["state"] = json!("dropped");
        batch.push(block);
    }
    batch
}

pub fn mirror(hook: &Hook, caller: &Caller, port: u16) -> Result<()> {
    let Some(steps) = steps(hook) else {
        return Ok(());
    };
    task_writes::update(hook, caller, port, |doc| batch(doc, caller, &steps))
}

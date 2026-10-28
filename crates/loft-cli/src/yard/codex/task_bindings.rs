//! Explicit plan-step references never mint a replacement task or infer a title match.

use serde_json::Value;

use super::commands::Caller;
use super::task_writes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reference<'a> {
    Plain,
    Bound(&'a str),
    Invalid,
}

pub fn reference(text: &str) -> Reference<'_> {
    let Some(rest) = text.strip_prefix("[yard:") else {
        return Reference::Plain;
    };
    let Some((id, _)) = rest.split_once(']') else {
        return Reference::Invalid;
    };
    if yardlog::Handle::parse(id).is_ok() {
        Reference::Bound(id)
    } else {
        Reference::Invalid
    }
}

pub fn batch(doc: &Value, caller: &Caller, steps: &[(&str, &str)]) -> Vec<Value> {
    steps
        .iter()
        .filter_map(|(text, state)| {
            let Reference::Bound(id) = reference(text) else {
                return None;
            };
            let (stage, held) = task_writes::tasks(doc).find(|(_, block)| block["id"] == id)?;
            let body = &held["version"]["body"];
            if !task_writes::executable(body, caller)
                || (!body["assignee"].is_object() && !caller.owns(&stage["owner"]))
                || matches!(body["state"].as_str(), Some("done" | "dropped"))
            {
                return None;
            }
            task_writes::changed(stage, held, state)
        })
        .collect()
}

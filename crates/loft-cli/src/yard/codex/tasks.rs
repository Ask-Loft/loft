//! Successful native plan updates become tasks, with stable text-and-occurrence identities.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::Result;
use super::commands::{self, Caller};
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
    hook.0["tool_input"]["plan"]
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
        .collect()
}

fn digest(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))[..24].to_owned()
}

fn batch(doc: &Value, caller: &Caller, steps: &[(&str, &str)]) -> Vec<Value> {
    let Some(stage) = commands::active(doc, caller) else {
        return vec![];
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
    let mut seen = BTreeSet::new();
    let mut batch = vec![];
    for (text, status) in steps {
        let occurrence = occurrences.entry(*text).or_insert(0usize);
        let id = format!("{prefix}{}", digest(&json!([text, occurrence])));
        *occurrence += 1;
        seen.insert(id.clone());
        let body = existing
            .get(id.as_str())
            .map(|block| &block["version"]["body"]);
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
        if seen.contains(id) || matches!(body["state"].as_str(), Some("done" | "dropped")) {
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
    for _ in 0..3 {
        let doc = commands::read(hook.dir(), caller, port)?;
        let batch = batch(&doc, caller, &steps);
        if batch.is_empty() {
            return Ok(());
        }
        let mut args = vec![
            "write".into(),
            "--input".into(),
            "-".into(),
            "--based-on".into(),
            doc["revision"].to_string(),
        ];
        args.extend(caller.args());
        let written = commands::run(hook.dir(), &args, port, Some(&json!(batch)))?;
        if written["outcome"] == "written" {
            return Ok(());
        }
        if !written["errors"]
            .as_array()
            .is_some_and(|errors| errors.iter().any(|error| error["kind"] == "stale"))
        {
            return Err(format!("Yard refused a task mirror: {}", written["errors"]));
        }
    }
    Err("Yard changed during three task mirror attempts".to_owned())
}

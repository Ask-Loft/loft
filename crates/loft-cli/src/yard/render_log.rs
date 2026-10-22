//! A whole Yard Log as `loft yard read` gives it: its root, its stages in order, each with its
//! blocks in order, and its open submissions, listed apart for each agent that sent them.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use yardlog::YardLog;
use yardlog::query::State;

use super::caller::{Continuations, Identity};
use super::render::{act, actor, block};

/// What a read returns of Yard Log.
pub struct Selection {
    /// Only the blocks that reached this copy after this revision.
    pub since: Option<u64>,
    /// Every version of each block, oldest first.
    pub every_version: bool,
}

/// Yard Log's root: its key, title, branch, format, the Act that opened it, and how it closed.
pub fn root(log: &YardLog, closed_by_merge: bool) -> Value {
    let root = &log.root;
    let mut fields = Map::new();
    fields.insert("key".to_owned(), json!(root.key.to_string()));
    fields.insert("title".to_owned(), json!(root.title));
    fields.insert("branch".to_owned(), json!(root.branch));
    if let Some(opened_as) = &root.opened_as {
        fields.insert("openedAs".to_owned(), json!(opened_as));
    }
    fields.insert("format".to_owned(), json!(root.format.to_string()));
    fields.insert("opened".to_owned(), act(&root.opened));
    if let Some(closed) = &root.closed {
        fields.insert("closed".to_owned(), act(closed));
    }
    fields.insert(
        "closedBy".to_owned(),
        json!(closed_by(log.is_closed(), closed_by_merge)),
    );
    let merged: Vec<String> = root.merged.iter().map(ToString::to_string).collect();
    fields.insert("merged".to_owned(), json!(merged));
    Value::Object(fields)
}

/// How Yard Log is closed: by an agent, by the merge into the base branch, or not at all.
pub const fn closed_by(closed: bool, by_merge: bool) -> &'static str {
    match (closed, by_merge) {
        (true, _) => "agent",
        (false, true) => "merge",
        (false, false) => "open",
    }
}

/// The stages in order, each with the blocks the selection returns; a stage is returned where it
/// changed after `since` or holds a block that did.
pub fn stages(log: &YardLog, selection: &Selection) -> Vec<Value> {
    let after = |reached: u64| selection.since.is_none_or(|since| reached > since);
    let mut stages = Vec::new();
    for file in &log.stages {
        let blocks: Vec<Value> = file
            .blocks
            .iter()
            .filter(|b| after(b.reached()))
            .map(|b| block(b, Some(&file.stage), selection.every_version))
            .collect();
        if blocks.is_empty() && !after(file.stage.reached()) {
            continue;
        }
        let mut stage = block(&file.stage, None, selection.every_version);
        stage["blocks"] = Value::Array(blocks);
        stages.push(stage);
    }
    stages
}

/// The open submissions, listed apart for each agent they count as sent by.
pub fn submissions(log: &YardLog, continuations: &Continuations) -> Vec<Value> {
    let mut by_agent: BTreeMap<Option<Identity>, (Value, Vec<Value>)> = BTreeMap::new();
    for submission in &log.root.submissions {
        if submission.state(log) != State::Open {
            continue;
        }
        let sender = submission.sender();
        let entry = by_agent
            .entry(continuations.identity_of(sender))
            .or_insert_with(|| (actor(sender), Vec::new()));
        let blocks: Vec<String> = submission.blocks.iter().map(ToString::to_string).collect();
        entry.1.push(json!({
            "key": submission.key.to_string(),
            "state": State::Open.as_str(),
            "blocks": blocks,
            "sent": act(submission.sending()),
        }));
    }
    by_agent
        .into_values()
        .map(|(agent, open)| json!({ "agent": agent, "open": open }))
        .collect()
}

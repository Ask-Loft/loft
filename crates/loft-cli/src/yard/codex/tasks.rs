//! Successful native plan updates become tasks of the agent's working stage, with stable
//! text-and-occurrence identities, applied to Yard Log as it stands under its lock.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use yardlog::body::{Body, Task, TaskState, TaskType};
use yardlog::input::Input;
use yardlog::log::{Block, StageFile};
use yardlog::task_change::input;
use yardlog::{Agent, Handle, YardLog};

use super::Result;
use super::commands::{self, Caller};
use super::task_bindings::{self, Reference};
use super::task_writes::wrote;
use super::translate::Hook;

fn successful(response: &Value) -> bool {
    response
        .as_str()
        .is_some_and(|text| text.trim() == "Plan updated")
        || response["success"] == true
        || (response["isError"] == false && response["content"].is_array())
}

fn steps(hook: &Hook) -> Option<Vec<(&str, TaskState)>> {
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
                "pending" => TaskState::Open,
                "in_progress" => TaskState::Active,
                "completed" => TaskState::Done,
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

/// The task block of `block`, where it is one.
fn task_of(block: &Block) -> Option<&Task> {
    match &block.current().body {
        Body::Task(task) => Some(task),
        _ => None,
    }
}

/// The changes a plan makes: its bound steps' tasks, and its plain steps mirrored into the
/// agent's working stage, each task new or changed as it stands; a task the plan no longer
/// holds that the agent may still change is dropped. A task new in a stage the agent does not
/// own is assigned to it.
fn batch(log: &YardLog, caller: &Caller, steps: &[(&str, TaskState)]) -> Vec<Input> {
    let agent = caller.agent();
    let mut batch = task_bindings::batch(log, &agent, steps);
    let Some(file) = log
        .working_stage(&agent, None)
        .and_then(|key| log.stages.iter().find(|file| file.stage.key.stage() == key))
    else {
        return batch;
    };
    let prefix = format!(
        "codex-task-{}-",
        digest(&json!([
            caller.session,
            caller.agent,
            file.stage.key.to_string()
        ]))
    );
    let existing: BTreeMap<&str, (&Handle, &Task)> = file
        .blocks
        .iter()
        .filter(|block| block.id.as_str().starts_with(&prefix))
        .filter_map(|block| Some((block.id.as_str(), (&block.id, task_of(block)?))))
        .collect();
    let mut seen: BTreeSet<String> = steps
        .iter()
        .filter_map(|(text, _)| match task_bindings::reference(text) {
            Reference::Bound(id) => Some(id.to_owned()),
            _ => None,
        })
        .collect();
    let mut occurrences = BTreeMap::new();
    for (text, state) in steps {
        if task_bindings::reference(text) != Reference::Plain {
            continue;
        }
        let occurrence = occurrences.entry(*text).or_insert(0usize);
        let id = format!("{prefix}{}", digest(&json!([text, occurrence])));
        *occurrence += 1;
        if !seen.insert(id.clone()) {
            continue;
        }
        let change = match existing.get(id.as_str()) {
            Some((handle, _)) => task_bindings::mirror(log, &agent, handle, *state),
            None => created(file, &agent, &id, text, *state),
        };
        batch.extend(change);
    }
    for (id, (handle, _)) in existing {
        if !seen.contains(id) {
            batch.extend(task_bindings::mirror(
                log,
                &agent,
                handle,
                TaskState::Dropped,
            ));
        }
    }
    batch
}

/// The task a new plain step makes, assigned to the agent where it does not own the stage.
fn created(
    file: &StageFile,
    agent: &Agent,
    id: &str,
    text: &str,
    state: TaskState,
) -> Option<Input> {
    let owns = file.stage.owner().is_some_and(|owner| owner.same(agent));
    let task = Task {
        title: text.to_owned(),
        detail: None,
        state,
        kind: TaskType::Regular,
        assignee: (!owns).then(|| agent.clone()),
    };
    Some(input(&Handle::parse(id).ok()?, &file.stage.id, task))
}

/// Mirrors a successful plan update.
pub fn mirror(hook: &Hook, caller: &Caller, port: u16) -> Result<()> {
    let Some(steps) = steps(hook) else {
        return Ok(());
    };
    commands::change_tasks(hook.dir(), caller, port, |log, context| {
        let change = log.change_tasks(batch(log, caller, &steps), context)?;
        Ok((wrote(&change), ()))
    })
    .map(|_| ())
}

//! Explicit plan-step references never mint a replacement task or infer a title match.

use yardlog::body::TaskState;
use yardlog::input::Input;
use yardlog::task_change::TaskPatch;
use yardlog::{Agent, Handle, YardLog};

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
    if Handle::parse(id).is_ok() {
        Reference::Bound(id)
    } else {
        Reference::Invalid
    }
}

/// The changes of the tasks the bound `steps` name, each to its step's state as the task
/// stands, under the mirror rule `loft yard task --mirror` applies.
pub fn batch(log: &YardLog, agent: &Agent, steps: &[(&str, TaskState)]) -> Vec<Input> {
    steps
        .iter()
        .filter_map(|(text, state)| {
            let Reference::Bound(id) = reference(text) else {
                return None;
            };
            mirror(log, agent, &Handle::parse(id).ok()?, *state)
        })
        .collect()
}

/// The change that mirrors `state` onto the task `id` as `agent`, where the mirror rule
/// applies it.
pub fn mirror(log: &YardLog, agent: &Agent, id: &Handle, state: TaskState) -> Option<Input> {
    let patch = TaskPatch {
        state: Some(state),
        mirror: true,
        ..TaskPatch::default()
    };
    log.patched(id, &patch, agent).ok().flatten()
}

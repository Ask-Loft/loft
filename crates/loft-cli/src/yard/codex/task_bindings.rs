//! Explicit plan-step references never mint a replacement task or infer a title match.

use yardlog::body::{Body, TaskState};
use yardlog::input::Input;
use yardlog::task_change::input;
use yardlog::{Agent, Handle, YardLog};

use super::task_writes::{finished, foreign, owns};

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
/// stands. A task another agent executes, an unassigned task of a stage the agent does not own
/// and a finished task are left as they are.
pub fn batch(log: &YardLog, agent: &Agent, steps: &[(&str, TaskState)]) -> Vec<Input> {
    steps
        .iter()
        .filter_map(|(text, state)| {
            let Reference::Bound(id) = reference(text) else {
                return None;
            };
            let (file, block) = log.stages.iter().find_map(|file| {
                let block = file.blocks.iter().find(|block| block.id.as_str() == id)?;
                Some((file, block))
            })?;
            let Body::Task(task) = &block.current().body else {
                return None;
            };
            if foreign(task, agent)
                || (task.assignee.is_none() && !owns(file, agent))
                || finished(task)
                || task.state == *state
            {
                return None;
            }
            let mut next = task.clone();
            next.state = *state;
            Some(input(&block.id, &file.stage.id, next))
        })
        .collect()
}

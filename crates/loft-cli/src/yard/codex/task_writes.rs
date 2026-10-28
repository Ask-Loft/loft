//! The adapter's changes of tasks, applied in-process to Yard Log as it stands under its lock,
//! as `loft yard task` applies them: no task is read before it is changed, and no change is
//! retried.

use serde_json::json;
use yardlog::Agent;
use yardlog::body::{Task, TaskState};
use yardlog::log::StageFile;
use yardlog::task_change::TaskChange;

use super::Result;
use super::commands::{self, Caller};
use super::translate::Hook;

pub fn actor_key(caller: &Caller) -> String {
    json!([caller.session, caller.agent.as_ref().map(|(id, _)| id)]).to_string()
}

/// Whether a change wrote.
pub const fn wrote(change: &TaskChange) -> bool {
    matches!(change, TaskChange::Changed { .. })
}

/// Whether a task is done or dropped.
pub const fn finished(task: &Task) -> bool {
    matches!(task.state, TaskState::Done | TaskState::Dropped)
}

/// Whether `agent` owns the stage of `file`.
pub fn owns(file: &StageFile, agent: &Agent) -> bool {
    file.stage.owner().is_some_and(|owner| owner.same(agent))
}

/// Whether an automatic mirror by `agent` leaves `task` alone: another agent executes it.
pub fn foreign(task: &Task, agent: &Agent) -> bool {
    task.assignee
        .as_ref()
        .is_some_and(|assignee| !assignee.same(agent))
}

/// Returns the caller's active tasks to open in one write, as `loft yard task --release` does.
pub fn release(hook: &Hook, caller: &Caller, port: u16) -> Result<()> {
    commands::change_tasks(hook.dir(), caller, port, |log, context| {
        let change = log.release(&context.agent, context)?;
        Ok((wrote(&change), ()))
    })
    .map(|_| ())
}

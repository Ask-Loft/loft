//! Changing a task in place: the fields an adapter names are applied to the task as Yard Log
//! holds it at the write, with no revision to base the change on, through the same write and
//! rules as any batch.

use crate::act::Agent;
use crate::body::{Body, Task, TaskState, TaskType};
use crate::input::{Fault, Input, Request};
use crate::keys::Handle;
use crate::log::YardLog;
use crate::write::{Context, Refusal, Reports, Written};

/// The fields a change of a task gives; a field it leaves `None` keeps its value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskPatch {
    pub state: Option<TaskState>,
    pub title: Option<String>,
    pub detail: Option<String>,
    pub kind: Option<TaskType>,
}

/// What a change of tasks did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskChange {
    /// It wrote a new version of each task it names, in one write.
    Changed {
        written: Written,
        tasks: Vec<Handle>,
    },
    /// It left every task as it was and wrote nothing.
    Unchanged,
}

impl YardLog {
    /// Applies `patch` to the task `id` as it stands, writing a new version that keeps its stage,
    /// its assignee and every field the patch does not give. A patch that leaves the task as it
    /// was writes nothing.
    ///
    /// # Errors
    ///
    /// Yard Log holds no task `id`, or the write is refused, as by the delegation rules.
    pub fn change_task(
        &mut self,
        id: &Handle,
        patch: &TaskPatch,
        context: &Context,
    ) -> Result<TaskChange, Refusal> {
        let (stage, current) = self.task(id).ok_or_else(|| no_task(id))?;
        let mut task = current.clone();
        if let Some(state) = patch.state {
            task.state = state;
        }
        if let Some(title) = &patch.title {
            task.title.clone_from(title);
        }
        if let Some(detail) = &patch.detail {
            task.detail = Some(detail.clone());
        }
        if let Some(kind) = patch.kind {
            task.kind = kind;
        }
        if &task == current {
            return Ok(TaskChange::Unchanged);
        }
        let input = input(id, stage, task);
        self.change(vec![input], context)
    }

    /// Returns each active task assigned to `agent` to `open`, keeping its assignee, in one
    /// write; with none, writes nothing.
    ///
    /// # Errors
    ///
    /// The write is refused.
    pub fn release(&mut self, agent: &Agent, context: &Context) -> Result<TaskChange, Refusal> {
        let inputs: Vec<Input> = self
            .assigned_to(agent)
            .filter(|(_, _, task)| task.state == TaskState::Active)
            .map(|(file, block, task)| {
                let open = Task {
                    state: TaskState::Open,
                    ..task.clone()
                };
                input(&block.id, &file.stage.id, open)
            })
            .collect();
        if inputs.is_empty() {
            return Ok(TaskChange::Unchanged);
        }
        self.change(inputs, context)
    }

    fn change(&mut self, inputs: Vec<Input>, context: &Context) -> Result<TaskChange, Refusal> {
        let context = Context {
            base: None,
            send: false,
            ..context.clone()
        };
        let written = self.write(&inputs, &context, &Reports::new())?;
        let tasks = inputs.into_iter().map(|input| input.id).collect();
        Ok(TaskChange::Changed { written, tasks })
    }

    /// The task `id`, with the id of the stage that holds it.
    fn task(&self, id: &Handle) -> Option<(&Handle, &Task)> {
        self.stages.iter().find_map(|file| {
            file.blocks
                .iter()
                .find(|block| &block.id == id)
                .and_then(|block| match &block.current().body {
                    Body::Task(task) => Some((&file.stage.id, task)),
                    _ => None,
                })
        })
    }
}

fn input(id: &Handle, stage: &Handle, task: Task) -> Input {
    Input {
        id: id.clone(),
        stage: Some(stage.clone()),
        after: None,
        request: Request::Task(task),
    }
}

fn no_task(id: &Handle) -> Refusal {
    Refusal::Faults(vec![Fault {
        block: id.to_string(),
        field: "id".to_owned(),
        message: format!("Yard Log holds no task `{id}`"),
    }])
}

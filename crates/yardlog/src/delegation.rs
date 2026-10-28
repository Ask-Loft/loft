//! Task execution identity, assignment guards and spans derived from immutable versions.

use crate::body::{Body, Status, Task, TaskState};
use crate::input::{Fault, Input, Request};
use crate::log::{Block, StageFile};
use crate::write::{Context, fault};
use crate::{Agent, StageKey, Time, YardLog};

fn unfinished(state: TaskState) -> bool {
    matches!(state, TaskState::Open | TaskState::Active)
}

fn same(left: Option<&Agent>, right: Option<&Agent>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.same(right),
        (None, None) => true,
        _ => false,
    }
}

pub(crate) fn faults(
    log: &YardLog,
    inputs: &[Input],
    input: &Input,
    context: &Context,
) -> Vec<Fault> {
    let Request::Task(next) = &input.request else {
        return vec![];
    };
    let Some(stage) = input.stage.as_ref() else {
        return vec![];
    };
    let held = log.stages.iter().find(|file| file.stage.id == *stage);
    let owner = held
        .and_then(|file| file.stage.owner())
        .is_some_and(|owner| owner.same(&context.agent))
        || inputs.iter().any(|input| {
            input.id == *stage
                && matches!(&input.request, Request::Stage(body) if body.status == Status::Active)
        });
    let previous = log.find(&input.id);
    let before = previous.and_then(|block| {
        if let Body::Task(task) = &block.current().body {
            Some(task)
        } else {
            None
        }
    });
    let assigned = before
        .and_then(|task| task.assignee.as_ref())
        .is_some_and(|agent| agent.same(&context.agent));
    let claim = before.is_none_or(|task| task.assignee.is_none() && unfinished(task.state))
        && next
            .assignee
            .as_ref()
            .is_some_and(|agent| agent.same(&context.agent));
    let mut faults = vec![];
    if !same(
        before.and_then(|task| task.assignee.as_ref()),
        next.assignee.as_ref(),
    ) && !owner
        && !assigned
        && !claim
    {
        faults.push(fault(input, "assignee", "only the stage owner or current assignee can reassign; claim an unassigned unfinished task only for yourself"));
    }
    let protected = previous.is_some_and(|block| {
        block
            .versions
            .iter()
            .any(|version| matches!(&version.body, Body::Task(task) if task.assignee.is_some()))
    });
    if protected
        && before.is_some_and(|task| task.state != next.state)
        && !owner
        && !assigned
        && !claim
    {
        faults.push(fault(input, "state", "only the current assignee or stage owner can change this delegated task's execution state"));
    }
    faults
}

/// One active execution attempt, closed on pause, completion or assignment handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution {
    pub started: Time,
    pub ended: Option<Time>,
    pub assignee: Option<Agent>,
}

impl Block {
    /// Derives execution attempts without changing or duplicating the task's history.
    #[must_use]
    pub fn executions(&self) -> Vec<Execution> {
        let mut spans: Vec<Execution> = vec![];
        for version in self.ordered() {
            let Body::Task(task) = &version.body else {
                return vec![];
            };
            let active = spans.last().filter(|span| span.ended.is_none());
            let unchanged = active.is_some_and(|span| {
                task.state == TaskState::Active
                    && same(span.assignee.as_ref(), task.assignee.as_ref())
            });
            if unchanged {
                continue;
            }
            if let Some(span) = spans.last_mut().filter(|span| span.ended.is_none()) {
                span.ended = Some(version.act.at);
            }
            if task.state == TaskState::Active {
                spans.push(Execution {
                    started: version.act.at,
                    ended: None,
                    assignee: task.assignee.clone(),
                });
            }
        }
        spans
    }
}

/// Whether an identified agent has no delegated execution, one stage, or ambiguous stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageContext {
    None,
    Stage(StageKey),
    Ambiguous,
}

impl YardLog {
    /// The current tasks explicitly assigned to this exact agent identity.
    pub fn assigned_to<'a>(
        &'a self,
        agent: &'a Agent,
    ) -> impl Iterator<Item = (&'a StageFile, &'a Block, &'a Task)> + 'a {
        self.stages.iter().flat_map(move |file| {
            file.blocks
                .iter()
                .filter_map(move |block| match &block.current().body {
                    Body::Task(task)
                        if task.assignee.as_ref().is_some_and(|who| who.same(agent)) =>
                    {
                        Some((file, block, task))
                    }
                    _ => None,
                })
        })
    }

    /// A unique stage from active assignments; never guesses between distinct stages.
    #[must_use]
    pub fn delegation_context(&self, agent: &Agent) -> StageContext {
        let mut context = StageContext::None;
        for (file, _, _) in self
            .assigned_to(agent)
            .filter(|(_, _, task)| task.state == TaskState::Active)
        {
            let stage = file.stage.key.stage();
            match context {
                StageContext::None => context = StageContext::Stage(stage),
                StageContext::Stage(previous) if previous == stage => {}
                _ => return StageContext::Ambiguous,
            }
        }
        context
    }
}

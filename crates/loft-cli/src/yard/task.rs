//! `loft yard task` (the Change a task in place Feature): the fields given applied to a task as
//! Yard Log holds it under its lock, with no revision to base the change on; or, with
//! `--release`, the calling agent's active tasks returned to `open` in one write. A change that
//! leaves every task as it was writes nothing.

use std::path::Path;

use serde_json::{Value, json};
use yardlog::body::{TaskState, TaskType};
use yardlog::task_change::{TaskChange, TaskPatch};
use yardlog::write::{Context, Refusal};
use yardlog::{Actor, Handle, YardLog};

use super::args::TaskArgs;
use super::branch::{self, Branch, Unapplied};
use super::caller::Caller;
use super::failure::{self, Failure};
use crate::finish::Finished;

const SCHEMA: &str = "loft-yard-task/1";

/// What the command changes.
enum Target {
    Task(Handle, TaskPatch),
    Release,
}

/// What a change did, at the revision it left Yard Log at.
struct Done {
    revision: u64,
    /// The task changed, by its id, with its state after the change.
    task: Option<(Handle, TaskState)>,
    /// The tasks released.
    released: Option<Vec<Handle>>,
}

/// Why a change wrote nothing: refused, or with nothing to change.
enum Stop {
    Refused(Refusal),
    Absent,
    Unchanged(Done),
}

pub fn run(root: &Path, args: TaskArgs) -> Finished {
    let as_json = args.output.json();
    let done = attempt(root, args);
    failure::end(SCHEMA, as_json, done, |outcome, fields| {
        let what = match (&fields["task"], &fields["released"]) {
            (Value::String(task), _) => format!("Task `{task}` is {}", fields["state"]),
            (_, Value::Array(ids)) if !ids.is_empty() => format!("Released {}", words(ids)),
            _ => "No active task to release".to_owned(),
        };
        format!(
            "{what} ({outcome}); Yard Log at revision {}",
            fields["revision"]
        )
    })
}

fn words(ids: &[Value]) -> String {
    let ids: Vec<String> = ids
        .iter()
        .map(|id| format!("`{}`", id.as_str().unwrap_or_default()))
        .collect();
    ids.join(", ")
}

fn attempt(dir: &Path, args: TaskArgs) -> Result<(&'static str, Value), Failure> {
    let target = target(&args)?;
    let caller = Caller::new(&args.writer.into());
    let branch = Branch::locate(dir, args.beyond)?;
    let at = super::now()?;
    let agent = caller.agent().clone();
    let settler = agent.clone();
    let held = branch.open(at, || Ok(Actor::Agent(settler)))?;
    branch.refuse_if_merged(&held)?;
    let applied = branch.update(|current| {
        let context = Context {
            at: super::now().map_err(Unapplied::Failed)?,
            agent,
            base: None,
            send: false,
        };
        apply(current, &target, &context).map_err(Unapplied::Refused)
    })?;
    let (outcome, done) = match applied {
        Ok(done) => (outcome(&target), done),
        Err(Stop::Unchanged(done)) => ("unchanged", done),
        Err(Stop::Absent) => return Err(branch.refused(&held, vec![branch::absent()])),
        Err(Stop::Refused(refusal)) => {
            return Err(super::write::refusal_of(
                &branch,
                &branch.store.read()?,
                &refusal,
            ));
        }
    };
    let mut fields = branch.fields(done.revision);
    if let Some((task, state)) = done.task {
        fields["task"] = json!(task.as_str());
        fields["state"] = json!(state.as_str());
    }
    if let Some(released) = done.released {
        fields["released"] = json!(released.iter().map(Handle::as_str).collect::<Vec<_>>());
    }
    Ok((outcome, fields))
}

const fn outcome(target: &Target) -> &'static str {
    match target {
        Target::Task(..) => "changed",
        Target::Release => "released",
    }
}

/// The change the arguments ask for; an id that cannot name a block is a usage error.
fn target(args: &TaskArgs) -> Result<Target, Failure> {
    let Some(id) = &args.id else {
        return Ok(Target::Release);
    };
    let id = Handle::parse(id).map_err(|error| Failure::Usage(format!("<ID>: {error}")))?;
    let patch = TaskPatch {
        state: (args.state.as_deref()).map(|word| TaskState::parse(word).expect("clap checks it")),
        title: args.title.clone(),
        detail: args.detail.clone(),
        kind: (args.kind.as_deref()).map(|word| TaskType::parse(word).expect("clap checks it")),
    };
    Ok(Target::Task(id, patch))
}

/// Applies the change under Yard Log's lock to Yard Log as it is: what it wrote, or why it
/// wrote nothing.
fn apply(
    current: Option<YardLog>,
    target: &Target,
    context: &Context,
) -> Result<(YardLog, Done), Stop> {
    let Some(mut log) = current else {
        return match target {
            Target::Task(..) => Err(Stop::Absent),
            Target::Release => Err(Stop::Unchanged(Done {
                revision: 0,
                task: None,
                released: Some(Vec::new()),
            })),
        };
    };
    log.upgrade_format();
    let change = match target {
        Target::Task(id, patch) => log.change_task(id, patch, context),
        Target::Release => log.release(&context.agent, context),
    }
    .map_err(Stop::Refused)?;
    let (changed, tasks) = match change {
        TaskChange::Changed { tasks, .. } => (true, tasks),
        TaskChange::Unchanged => (false, Vec::new()),
    };
    let done = Done {
        revision: log.root.revision,
        task: match target {
            Target::Task(id, _) => Some((id.clone(), state_of(&log, id))),
            Target::Release => None,
        },
        released: matches!(target, Target::Release).then_some(tasks),
    };
    if changed {
        Ok((log, done))
    } else {
        Err(Stop::Unchanged(done))
    }
}

fn state_of(log: &YardLog, id: &Handle) -> TaskState {
    match log.find(id).map(|block| &block.current().body) {
        Some(yardlog::body::Body::Task(task)) => task.state,
        _ => unreachable!("a task changed or left as it was is a task"),
    }
}

//! `loft yard task` (the Change a task in place Feature): the fields given applied to a task as
//! Yard Log holds it under its lock, with no revision to base the change on; or, with
//! `--release`, the calling agent's active tasks returned to `open` in one write. A change that
//! leaves every task as it was writes nothing.

use std::path::Path;

use serde_json::{Value, json};
use yardlog::body::{TaskState, TaskType};
use yardlog::task_change::{TaskChange, TaskPatch};
use yardlog::write::{Context, Refusal};
use yardlog::{Actor, Agent, Handle, YardLog};
use yardlog_store::Held;

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

/// Why a change wrote nothing: refused, with no Yard Log to change, or with nothing to change.
pub(super) enum Stop<T> {
    Refused(Refusal),
    Absent,
    Unchanged(T),
}

/// What a change applied to Yard Log as it stands: the value it gives, with whether it wrote.
pub(super) type Change<T> = Result<(bool, T), Refusal>;

/// What a change under the lock left: Yard Log as it was held before, and the change's value or
/// why it wrote nothing.
pub(super) type Locked<T> = (Held, Result<T, Stop<T>>);

/// Applies `change` as `agent` to the branch's Yard Log as it stands under its lock, with no
/// revision to base it on: Yard Log as it was held before, and the value the change gave, or why
/// it wrote nothing. The `loft yard task` command and the adapters that change tasks in-process
/// share it.
pub(super) fn under_lock<T>(
    branch: &Branch,
    agent: &Agent,
    change: impl FnOnce(&mut YardLog, &Context) -> Change<T>,
) -> Result<Locked<T>, Failure> {
    let settler = agent.clone();
    let held = branch.open(super::now()?, || Ok(Actor::Agent(settler)))?;
    branch.refuse_if_merged(&held)?;
    let applied = branch.update(|current| {
        let Some(mut log) = current else {
            return Err(Unapplied::Refused(Stop::Absent));
        };
        let context = Context {
            at: super::now().map_err(Unapplied::Failed)?,
            agent: agent.clone(),
            base: None,
            send: false,
        };
        log.upgrade_format();
        match change(&mut log, &context) {
            Ok((true, value)) => Ok((log, value)),
            Ok((false, value)) => Err(Unapplied::Refused(Stop::Unchanged(value))),
            Err(refusal) => Err(Unapplied::Refused(Stop::Refused(refusal))),
        }
    })?;
    Ok((held, applied))
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
    let (held, applied) = under_lock(&branch, caller.agent(), |log, context| {
        apply(log, &target, context)
    })?;
    let (outcome, done) = match applied {
        Ok(done) => (outcome(&target), done),
        Err(Stop::Unchanged(done)) => ("unchanged", done),
        Err(Stop::Absent) if matches!(target, Target::Release) => (
            "unchanged",
            Done {
                revision: 0,
                task: None,
                released: Some(Vec::new()),
            },
        ),
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

/// Applies the change to Yard Log as it is under its lock: what it did, and whether it wrote.
fn apply(log: &mut YardLog, target: &Target, context: &Context) -> Change<Done> {
    let change = match target {
        Target::Task(id, patch) => log.change_task(id, patch, context),
        Target::Release => log.release(&context.agent, context),
    }?;
    let (changed, tasks) = match change {
        TaskChange::Changed { tasks, .. } => (true, tasks),
        TaskChange::Unchanged => (false, Vec::new()),
    };
    let done = Done {
        revision: log.root.revision,
        task: match target {
            Target::Task(id, _) => Some((id.clone(), state_of(log, id))),
            Target::Release => None,
        },
        released: matches!(target, Target::Release).then_some(tasks),
    };
    Ok((changed, done))
}

fn state_of(log: &YardLog, id: &Handle) -> TaskState {
    match log.find(id).map(|block| &block.current().body) {
        Some(yardlog::body::Body::Task(task)) => task.state,
        _ => unreachable!("a task changed or left as it was is a task"),
    }
}

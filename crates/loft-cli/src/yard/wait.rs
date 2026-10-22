//! `loft yard wait` (the Wait for the answers Feature): returns once a submission is no longer
//! open, with the answers and outcomes of its blocks, or `pending` once its timeout passes. It
//! holds the submission's lease for as long as it runs, so that one agent at a time waits on it;
//! the main agent of another session takes up a submission whose session stopped by waiting on
//! it. It reads Yard Log every 250 milliseconds and changes it only to record a take-up.

use std::path::Path;
use std::time::Duration;

use loft_ports::clock::Clock;

use serde_json::{Value, json};
use yardlog::log::Submission;
use yardlog::query::State;
use yardlog::respond::Refused;
use yardlog::{Agent, Key, SubmissionKey, YardLog};
use yardlog_store::home::Home;
use yardlog_store::lock::Locked;
use yardlog_store::{Held, YardStoreError};

use super::args::WaitArgs;
use super::branch::{Branch, log_of, revision_of};
use super::caller::{Caller, Continuations};
use super::change::refusal;
use super::failure::{self, Failure};
use super::render;
use super::standing::{Standing, standing};
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-wait/1";

/// The longest one wait lasts, within the ten minutes a host command may run.
pub const LONGEST: u64 = 540;

/// How often a wait reads Yard Log.
const EVERY: Duration = Duration::from_millis(250);

/// How often a wait asks git whether the base branch came to hold its Yard Log, as a merge of
/// its branch makes it: a few git calls a minute, however long the wait.
const MERGE_EVERY: Duration = Duration::from_secs(5);

pub fn run(root: &Path, args: WaitArgs) -> Finished {
    let as_json = args.output.json();
    match attempt(root, args) {
        Ok((outcome, fields)) => {
            if as_json {
                json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
            } else {
                outln!(
                    "Submission {} is {outcome}; Yard Log at revision {}",
                    fields["submission"].as_str().unwrap_or_default(),
                    fields["revision"]
                );
            }
            Finished::success(outcome)
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}

fn attempt(dir: &Path, args: WaitArgs) -> Result<(&'static str, Value), Failure> {
    let submission = Key::parse(&args.submission)
        .map(SubmissionKey::new)
        .map_err(|error| Failure::Usage(format!("--submission: {error}")))?;
    let timeout = Duration::from_secs(args.timeout.min(LONGEST));
    let clock = super::clock();
    let started = clock.elapsed();
    let deadline = started + timeout;
    let caller = Caller::new(args.writer.into());
    let branch = Branch::locate(dir, args.beyond)?;
    let home = Home::from_env();
    let at = super::now()?;
    caller.arrive(home.as_ref(), at)?;
    let agent = caller.agent().clone();
    let settler = agent.clone();
    let held = branch.open(at, || Ok(yardlog::Actor::Agent(settler)))?;
    let closed_by_merge =
        log_of(&held).is_some_and(|log| !log.is_closed()) && branch.closed_by_merge()?;
    let waiter = Waiter {
        branch: &branch,
        key: submission,
        agent,
        home,
    };
    let taking_up = waiter.may_wait(&held)?;
    let _lease = waiter.lease()?;
    // Another wait may have taken the submission up while this one took the lease.
    let taking_up = taking_up && waiter.may_wait(&branch.store.read()?)?;
    let taken_up = taking_up && waiter.take_up(at)?;
    let (outcome, mut fields) = waiter.until(clock, deadline, closed_by_merge)?;
    fields["takenUp"] = json!(taken_up);
    fields["waited"] = json!(clock.elapsed().saturating_sub(started).as_secs());
    Ok((outcome, fields))
}

/// A wait on one submission by the calling agent.
struct Waiter<'a> {
    branch: &'a Branch,
    key: SubmissionKey,
    agent: Agent,
    home: Option<Home>,
}

impl Waiter<'_> {
    /// Reads Yard Log every [`EVERY`] until the submission is no longer open or `deadline`
    /// passes, and asks git every [`MERGE_EVERY`] whether the base branch came to hold Yard Log;
    /// gives the outcome and the submission's blocks. Refused where Yard Log leaves the tree.
    fn until(
        &self,
        clock: &impl Clock,
        deadline: Duration,
        mut merged: bool,
    ) -> Result<(&'static str, Value), Failure> {
        let mut checked = clock.elapsed();
        loop {
            let held = self.branch.store.read()?;
            let Some((log, found)) =
                log_of(&held).and_then(|log| Some((log, log.submission(self.key)?)))
            else {
                let message = format!(
                    "the Yard Log of `{}` is no longer in the tree, as after a checkout of another \
                     branch",
                    self.branch.store.branch()
                );
                return Err(self.refused(&held, failure::error("gone", &message)));
            };
            if !merged && !log.is_closed() && clock.elapsed().saturating_sub(checked) >= MERGE_EVERY
            {
                checked = clock.elapsed();
                merged = self.branch.closed_by_merge()?;
            }
            let outcome = if log.is_closed() || merged {
                Some("closed")
            } else {
                match found.state(log) {
                    State::Answered => Some("answered"),
                    State::Withdrawn => Some("withdrawn"),
                    State::Open => (clock.elapsed() >= deadline).then_some("pending"),
                }
            };
            if let Some(outcome) = outcome {
                return Ok((outcome, self.fields(log, found)));
            }
            clock.wait(EVERY.min(deadline.saturating_sub(clock.elapsed())));
        }
    }

    /// Whether the calling agent may wait on the submission as Yard Log holds it, and must take
    /// it up to: refused where it may not, or where Yard Log does not hold it.
    fn may_wait(&self, held: &Held) -> Result<bool, Failure> {
        let found = log_of(held).and_then(|log| log.submission(self.key));
        let Some(found) = found else {
            let message = format!("Yard Log holds no submission {}", self.key);
            return Err(self.refused(held, failure::error("no-submission", &message)));
        };
        let continuations = Continuations::read(self.home.as_ref())?;
        let at = super::now()?;
        match standing(found, &self.agent, &continuations, self.home.as_ref(), at)? {
            Standing::Own => Ok(false),
            Standing::TakeUp => Ok(true),
            Standing::Refused(error) => Err(self.refused(held, error)),
        }
    }

    /// The submission's lease, refused, naming who holds it, where another wait does.
    fn lease(&self) -> Result<Locked, Failure> {
        let place = self.branch.place();
        let holder = render::actor(&yardlog::Actor::Agent(self.agent.clone()));
        let bytes = serde_json::to_vec(&holder).expect("a JSON value serializes");
        if let Some(lease) = place.lease_as(self.key, &bytes)? {
            return Ok(lease);
        }
        let holder = place.lease_holder(self.key)?.unwrap_or_default();
        let holder = serde_json::from_slice::<Value>(&holder).unwrap_or(Value::Null);
        let message = format!(
            "another wait holds the lease on submission {}: {}",
            self.key,
            holder_text(&holder)
        );
        let held = self.branch.store.read()?;
        Err(self.refused(&held, failure::error("leased", &message)))
    }

    /// Records that the calling main agent took the submission up, and whether it still was
    /// open to take up.
    fn take_up(&self, at: yardlog::Time) -> Result<bool, Failure> {
        let taken = self.branch.store.update(|current| {
            // Gone from the tree since it was read: nothing is taken up, and the wait says so.
            let mut log = current.ok_or(Taken::Gone)?;
            let taken = match log.take_up(self.key, self.agent.clone(), at) {
                Ok(_) => true,
                // Answered, withdrawn or closed since: the wait returns that instead.
                Err(Refused::NotOpen { .. } | Refused::Closed) => false,
                Err(refused) => return Err(Taken::Refused(refused)),
            };
            Ok::<_, Taken>((log, taken))
        });
        match taken {
            Ok(updated) => Ok(updated.value),
            Err(Taken::Gone) => Ok(false),
            Err(Taken::Store(error)) => Err(error.into()),
            Err(Taken::Refused(refused)) => {
                let held = self.branch.store.read()?;
                Err(self.refused(&held, refusal(&refused)))
            }
        }
    }

    fn refused(&self, held: &Held, error: Value) -> Failure {
        let mut fields = self.branch.fields(revision_of(held));
        fields["submission"] = json!(self.key.to_string());
        Failure::Refused {
            fields,
            errors: vec![error],
        }
    }

    /// The result: where it acted, the revision, and the submission's blocks with their answers
    /// and outcomes.
    fn fields(&self, log: &YardLog, found: &Submission) -> Value {
        let mut fields = self.branch.fields(log.root.revision);
        fields["submission"] = json!(self.key.to_string());
        let blocks: Vec<Value> = found
            .blocks
            .iter()
            .filter_map(|key| log.block(*key))
            .map(|(file, block)| {
                let stage = (file.stage.key != block.key).then_some(&file.stage);
                render::block(block, stage, false)
            })
            .collect();
        fields["blocks"] = Value::Array(blocks);
        fields
    }
}

/// Why a take-up did not apply.
enum Taken {
    Store(YardStoreError),
    Refused(Refused),
    Gone,
}

impl From<YardStoreError> for Taken {
    fn from(error: YardStoreError) -> Self {
        Self::Store(error)
    }
}

/// The wait that holds a lease, for a person: its session and agent.
fn holder_text(holder: &Value) -> String {
    let session = holder["session"].as_str().unwrap_or("an unknown session");
    match (holder["agent"].as_str(), holder["agentType"].as_str()) {
        (Some(id), Some(kind)) => format!("{kind} {id} of session {session}"),
        _ => format!("session {session}"),
    }
}

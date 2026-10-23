//! `loft yard write` (the Write blocks Feature): a batch of blocks, written whole or not at all.
//! An agent's first write on a branch opens its Yard Log; a batch marked with `--send` opens a
//! submission of its questions, approvals and plans. Yard Log in an older format is written in
//! this `loft`'s format with the batch, as one write.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};
use yardlog::body::Recorded;
use yardlog::datum::Datum;
use yardlog::input::{Fault, Request, read_batch};
use yardlog::keys::Handle;
use yardlog::write::{Context, Refusal, Written};
use yardlog::{Format, YardLog};
use yardlog_store::{Held, YardStoreError};

use super::args::WriteArgs;
use super::branch::{Branch, log_of, revision_of};
use super::caller::Caller;
use super::failure::{self, Failure};
use super::render;
use super::views::Views;
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-write/1";
const OUTCOME: &str = "written";

/// A view's report, or why it could not be computed.
type Reported = Result<(Recorded, Datum), String>;

pub fn run(root: &Path, args: WriteArgs) -> Finished {
    let as_json = args.output.json();
    match attempt(root, args) {
        Ok(fields) => {
            if as_json {
                json::print(&json::result(SCHEMA, OUTCOME, fields, Vec::new()));
            } else {
                outln!(
                    "Yard Log of {} at revision {}; {}",
                    fields["branch"].as_str().unwrap_or_default(),
                    fields["revision"],
                    super::read::page_line(&fields["page"])
                );
            }
            Finished::success(OUTCOME)
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}

/// Why a write did not apply.
enum Unwritten {
    Store(YardStoreError),
    Refused(Refusal),
}

impl From<YardStoreError> for Unwritten {
    fn from(error: YardStoreError) -> Self {
        Self::Store(error)
    }
}

/// What a write applied: the batch's result, and the format Yard Log was upgraded from.
struct Applied {
    written: Written,
    upgraded_from: Option<Format>,
}

fn attempt(dir: &Path, args: WriteArgs) -> Result<Value, Failure> {
    let caller = Caller::new(&args.writer.into());
    let branch = Branch::locate(dir, args.beyond)?;
    let batch = super::input::json(&args.input)?;
    let at = super::now()?;
    let agent = caller.agent().clone();
    let held = branch.open(at, || Ok(yardlog::Actor::Agent(agent.clone())))?;
    let inputs = match read_batch(&batch) {
        Ok(inputs) => inputs,
        Err(faults) => return Err(refused(&branch, &held, faults.iter().map(fault).collect())),
    };
    if branch.merged(&held)? {
        return Err(refused(&branch, &held, vec![closed_by_merge()]));
    }
    let context = Context {
        at,
        agent: agent.clone(),
        base: args.based_on,
        send: args.send,
    };
    let branch_name = branch.store.branch().to_owned();
    // The views compare with the base branch Yard Log records, or the one this write opens it on.
    let base = log_of(&held).map_or(args.base.as_str(), |log| log.root.base.as_str());
    let views = Views {
        tree: dir,
        base,
        at,
    };
    // The views read the specification and git, and need nothing of Yard Log: each is computed
    // before Yard Log's lock is taken, so that no other command waits on them.
    let reports: BTreeMap<Handle, Reported> = inputs
        .iter()
        .filter_map(|input| match &input.request {
            Request::Loft(request) => Some((input.id.clone(), views.report(&input.id, request))),
            _ => None,
        })
        .collect();
    let applied = branch.store.update(|current| {
        let mut log =
            current.unwrap_or_else(|| YardLog::open(&branch_name, &args.base, at, agent.clone()));
        let upgraded_from = upgrade(&mut log);
        let written = log
            .write(&inputs, &context, &mut |block, _| reports[block].clone())
            .map_err(Unwritten::Refused)?;
        Ok::<_, Unwritten>((
            log,
            Applied {
                written,
                upgraded_from,
            },
        ))
    });
    match applied {
        Ok(updated) => Ok(result(&branch, &updated.value)),
        Err(Unwritten::Store(error)) => Err(error.into()),
        Err(Unwritten::Refused(refusal)) => {
            let held = branch.store.read()?;
            Err(refusal_of(&branch, &held, &refusal))
        }
    }
}

/// Brings Yard Log in an older format up to this `loft`'s, giving the format it was in.
fn upgrade(log: &mut YardLog) -> Option<Format> {
    let from = log.root.format;
    (from < Format::CURRENT).then(|| {
        log.root.format = Format::CURRENT;
        from
    })
}

fn result(branch: &Branch, applied: &Applied) -> Value {
    let written = &applied.written;
    let mut fields = branch.fields(written.revision);
    let blocks: Vec<Value> = written
        .blocks
        .iter()
        .map(|(id, key)| json!({ "id": id.as_str(), "key": key.to_string() }))
        .collect();
    fields["blocks"] = Value::Array(blocks);
    if let Some(submission) = written.submission {
        fields["submission"] = json!(submission.to_string());
    }
    if let Some(from) = applied.upgraded_from {
        fields["upgradedFrom"] = json!(from.to_string());
    }
    fields
}

/// A refusal's result: where it was refused, Yard Log's revision, and why.
fn refused(branch: &Branch, held: &Held, errors: Vec<Value>) -> Failure {
    Failure::Refused {
        fields: branch.fields(revision_of(held)),
        errors,
    }
}

fn fault(fault: &Fault) -> Value {
    let message = if fault.field.is_empty() {
        format!("block `{}`: {}", fault.block, fault.message)
    } else {
        format!(
            "block `{}`, field `{}`: {}",
            fault.block, fault.field, fault.message
        )
    };
    failure::error("block", &message)
}

pub fn closed_by_merge() -> Value {
    failure::error(
        "closed",
        "Yard Log is closed: the base branch holds it, so the merge closed it",
    )
}

/// The result of a refused batch: each fault, and for a stale batch each block changed since
/// its base revision, with its current version.
fn refusal_of(branch: &Branch, held: &Held, refusal: &Refusal) -> Failure {
    let message = refusal.to_string();
    let errors = match refusal {
        Refusal::Faults(faults) => faults.iter().map(fault).collect(),
        Refusal::Closed => vec![failure::error("closed", &message)],
        Refusal::Newer(_) => vec![failure::error("newer", &message)],
        Refusal::Stale { .. } => vec![failure::error("stale", &message)],
        Refusal::Held { .. } => vec![failure::error("held", &message)],
        Refusal::NothingToSend => vec![failure::error("nothing-to-send", &message)],
        Refusal::Report { .. } => vec![failure::error("view", &message)],
    };
    let mut fields = branch.fields(revision_of(held));
    if let (Refusal::Stale { blocks, .. }, Some(log)) = (refusal, log_of(held)) {
        let stale: Vec<Value> = blocks
            .iter()
            .filter_map(|key| log.block(*key))
            .map(|(file, block)| {
                let stage = (file.stage.key != block.key).then_some(&file.stage);
                render::block(block, stage, false)
            })
            .collect();
        fields["stale"] = Value::Array(stale);
    }
    Failure::Refused { fields, errors }
}

//! `loft yard write` (the Write blocks Feature): a batch of blocks, written whole or not at all.
//! An agent's first write on a branch opens its Yard Log; a batch marked with `--send` opens a
//! submission of its questions, approvals and plans. Yard Log in an older format is written in
//! this `loft`'s format with the batch, as one write.

use std::path::Path;

use serde_json::{Value, json};
use yardlog::input::{Fault, Request, read_batch};
use yardlog::write::{Context, Refusal, Reports, Written};
use yardlog::{Format, YardLog};
use yardlog_store::Held;

use super::args::{DEFAULT_BASE, WriteArgs};
use super::branch::{Branch, Unapplied, log_of, revision_of};
use super::caller::Caller;
use super::failure::{self, Failure};
use super::render;
use super::views::Views;
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-write/1";
const OUTCOME: &str = "written";

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
        Err(faults) => return Err(branch.refused(&held, faults.iter().map(fault).collect())),
    };
    branch.refuse_if_merged(&held)?;
    let context = Context {
        at,
        agent: agent.clone(),
        base: args.based_on,
        send: args.send,
    };
    let branch_name = branch.store.branch().to_owned();
    // The views compare with the base branch Yard Log records, or the one this write opens it on.
    let base = match log_of(&held) {
        Some(log) => log.root.base.clone(),
        None => opening_base(
            &branch,
            dir,
            args.base.as_deref(),
            args.default_base.as_deref(),
        )?,
    };
    let views = Views {
        tree: dir,
        base: &base,
        at,
    };
    // The views read the specification and git, and need nothing of Yard Log: each is computed
    // before Yard Log's lock is taken, so that no other command waits on them.
    let reports: Reports = inputs
        .iter()
        .filter_map(|input| match &input.request {
            Request::Loft(request) => Some((input.id.clone(), views.report(request))),
            _ => None,
        })
        .collect();
    let applied = branch.update(|current| {
        let mut log =
            current.unwrap_or_else(|| YardLog::open(&branch_name, &base, at, agent.clone()));
        // The write's own Act records the upgrade.
        let upgraded_from = log.upgrade_format();
        let written = log
            .write(&inputs, &context, &reports)
            .map_err(Unapplied::Refused)?;
        Ok((
            log,
            Applied {
                written,
                upgraded_from,
            },
        ))
    })?;
    match applied {
        Ok(applied) => Ok(result(&branch, &applied)),
        Err(refusal) => Err(refusal_of(&branch, &branch.store.read()?, &refusal)),
    }
}

/// The base branch a write records as it opens Yard Log: the one `--base` names; else the local
/// branch git says this one was cut from; else the one `--default-base` names; else `develop`.
fn opening_base(
    branch: &Branch,
    dir: &Path,
    named: Option<&str>,
    default: Option<&str>,
) -> Result<String, Failure> {
    if let Some(named) = named {
        return Ok(named.to_owned());
    }
    let cut_from = branch.git.cut_from(dir, branch.store.branch())?;
    Ok(cut_from.unwrap_or_else(|| default.unwrap_or(DEFAULT_BASE).to_owned()))
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
        fields["stale"] = render::blocks(log, blocks);
    }
    Failure::Refused { fields, errors }
}

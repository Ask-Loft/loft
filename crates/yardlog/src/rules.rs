//! The rules a batch is held to that need Yard Log: what an id may become, where a new block
//! goes, what a decision may name, and which blocks may change at all.

use std::collections::BTreeMap;

use crate::body::Kind;
use crate::input::{Fault, Input, Request};
use crate::keys::{BlockKey, Handle};
use crate::log::YardLog;
use crate::write::{Refusal, fault, holder};

/// The faults of one block of a batch.
pub(crate) fn faults(log: &YardLog, inputs: &[Input], input: &Input) -> Vec<Fault> {
    match log.find(&input.id) {
        Some(_) => rewrite(log, input),
        None => placement(log, inputs, input),
    }
    .into_iter()
    .chain(decision(log, inputs, input))
    .collect()
}

/// A block written again keeps its kind and its stage, and a `loft` block is never written
/// again.
fn rewrite(log: &YardLog, input: &Input) -> Vec<Fault> {
    let block = log.find(&input.id).expect("the caller found it");
    let kind = input.request.kind();
    if block.kind != kind {
        return vec![fault(
            input,
            "kind",
            format!(
                "`{}` is a `{}` block, and a block keeps its kind",
                input.id,
                block.kind.as_str()
            ),
        )];
    }
    if kind == Kind::Loft {
        return vec![fault(
            input,
            "id",
            "a `loft` block keeps the report it recorded; record a later report as a new block",
        )];
    }
    let (file, _) = log.block(block.key).expect("the block is held");
    match &input.stage {
        Some(stage) if *stage != file.stage.id => vec![fault(
            input,
            "stage",
            format!(
                "the block is held by stage `{}`, and a block keeps its stage",
                file.stage.id
            ),
        )],
        _ => Vec::new(),
    }
}

/// A new block names a stage of Yard Log or one written earlier in the batch, since a batch is
/// applied in order, and goes after a block of that stage.
fn placement(log: &YardLog, inputs: &[Input], input: &Input) -> Vec<Fault> {
    let earlier = || inputs.iter().take_while(|other| other.id != input.id);
    let is_stage = |id: &Handle| {
        log.find(id).is_some_and(|block| block.kind == Kind::Stage)
            || earlier().any(|other| &other.id == id && other.request.kind() == Kind::Stage)
    };
    let Some(stage) = &input.stage else {
        let placed = |after: &Handle| {
            log.find(after)
                .is_some_and(|block| block.kind == Kind::Stage)
                || earlier().any(|other| &other.id == after && other.request.kind() == Kind::Stage)
        };
        return match &input.after {
            Some(after) if !placed(after) => {
                vec![fault(
                    input,
                    "after",
                    format!("names no stage `{after}` of Yard Log"),
                )]
            }
            _ => Vec::new(),
        };
    };
    if !is_stage(stage) {
        return vec![fault(
            input,
            "stage",
            format!("names no stage `{stage}` of Yard Log"),
        )];
    }
    let held_by_stage = |after: &Handle| {
        let held = log.find(after).and_then(|block| log.block(block.key));
        held.is_some_and(|(file, block)| &file.stage.id == stage && block.kind != Kind::Stage)
            || earlier().any(|other| &other.id == after && other.stage.as_ref() == Some(stage))
    };
    match &input.after {
        Some(after) if !held_by_stage(after) => vec![fault(
            input,
            "after",
            format!("names no block `{after}` of stage `{stage}`"),
        )],
        _ => Vec::new(),
    }
}

/// A decision names a question of Yard Log or of the batch.
fn decision(log: &YardLog, inputs: &[Input], input: &Input) -> Option<Fault> {
    let Request::Decision {
        question: Some(question),
        ..
    } = &input.request
    else {
        return None;
    };
    let known = log
        .find(question)
        .is_some_and(|block| block.kind == Kind::Question)
        || inputs
            .iter()
            .any(|other| &other.id == question && other.request.kind() == Kind::Question);
    (!known).then(|| {
        fault(
            input,
            "question",
            format!("names no question `{question}` of Yard Log"),
        )
    })
}

/// Refuses a batch that writes again a block an open submission holds.
pub(crate) fn held(log: &YardLog, existing: &BTreeMap<Handle, BlockKey>) -> Result<(), Refusal> {
    for (id, key) in existing {
        if let Some(submission) = holder(log, *key) {
            return Err(Refusal::Held {
                block: id.clone(),
                submission,
            });
        }
    }
    Ok(())
}

/// Refuses a batch that writes again a block changed after the revision the batch is based on.
pub(crate) fn stale(
    log: &YardLog,
    base: u64,
    existing: &BTreeMap<Handle, BlockKey>,
) -> Result<(), Refusal> {
    let changed: Vec<BlockKey> = existing
        .values()
        .filter(|key| {
            log.block(**key)
                .is_some_and(|(_, block)| block.reached() > base)
        })
        .copied()
        .collect();
    if changed.is_empty() {
        Ok(())
    } else {
        Err(Refusal::Stale {
            base,
            revision: log.root.revision,
            blocks: changed,
        })
    }
}

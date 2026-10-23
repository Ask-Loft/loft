//! The merge of two copies of one file of Yard Log, as git's merge driver runs it: records join
//! by key, every version and every answer of either copy is kept, and what this copy did not
//! hold is marked as arriving. What spans files, ids shared by two records and the numbers of
//! decisions written apart, is left to the settling of the next command.

use crate::act::Act;
use crate::codec::{FileError, ROOT, session};
use crate::log::{Arrival, Block, Change, Root, StageFile, Submission};

/// Joins `theirs` into `ours`, this copy, over their common `base` where there is one.
#[must_use]
pub fn merge_root(base: Option<&Root>, ours: &Root, theirs: &Root) -> Root {
    let (first, other) = if (theirs.opened.at, theirs.key) < (ours.opened.at, ours.key) {
        (theirs, ours)
    } else {
        (ours, theirs)
    };
    let mut merged = ours.merged.clone();
    for key in theirs
        .merged
        .iter()
        .chain(std::iter::once(&other.key).filter(|key| **key != first.key))
    {
        if !merged.contains(key) {
            merged.push(*key);
        }
    }
    let closed = [&ours.closed, &theirs.closed]
        .into_iter()
        .flatten()
        .min_by_key(|act| (act.at, act.revision))
        .cloned();
    let new: Vec<Act> = theirs
        .acts
        .iter()
        .filter(|act| !ours.acts.contains(act))
        .cloned()
        .collect();
    let mut acts = ours.acts.clone();
    acts.extend(new.iter().cloned());
    let mut arriving = ours.arriving.clone();
    arriving.extend(new.into_iter().filter(|act| !ours.arriving.contains(act)));
    let mut settlings = ours.settlings.clone();
    settlings.extend(
        theirs
            .settlings
            .iter()
            .filter(|s| !ours.settlings.contains(s))
            .cloned(),
    );
    Root {
        format: ours.format.max(theirs.format),
        key: first.key,
        branch: ours.branch.clone(),
        base: first.base.clone(),
        opened_as: ours.opened_as.clone(),
        title: ours.title.clone(),
        revision: ours.revision.max(theirs.revision) + 1,
        opened: first.opened.clone(),
        stages: ordered(
            base.map(|root| root.stages.as_slice()),
            &ours.stages,
            &theirs.stages,
            |entry| entry.key,
        ),
        submissions: join(
            &ours.submissions,
            &theirs.submissions,
            |s| s.key,
            join_submission,
        ),
        merged,
        closed,
        acts,
        unsettled_since: Some(
            ours.unsettled_since
                .map_or(ours.revision, |since| since.min(ours.revision)),
        ),
        arriving,
        settlings,
    }
}

/// Joins `theirs` into `ours`, two copies of one stage's file.
#[must_use]
pub fn merge_stage(base: Option<&StageFile>, ours: &StageFile, theirs: &StageFile) -> StageFile {
    let blocks = join(&ours.blocks, &theirs.blocks, |b| b.key, join_block);
    let order = ordered(
        base.map(|file| file.blocks.as_slice()),
        &ours.blocks,
        &theirs.blocks,
        |block| block.key,
    );
    StageFile {
        stage: join_block(&ours.stage, &theirs.stage),
        blocks: order
            .into_iter()
            .map(|block| {
                blocks
                    .iter()
                    .find(|b| b.key == block.key)
                    .expect("joined")
                    .clone()
            })
            .collect(),
    }
}

/// The base's records in its order, then each copy's new records in their order within it.
fn ordered<T: Clone, K: PartialEq>(
    base: Option<&[T]>,
    ours: &[T],
    theirs: &[T],
    key: impl Fn(&T) -> K,
) -> Vec<T> {
    let mut result: Vec<T> = Vec::new();
    let has = |result: &[T], item: &T| result.iter().any(|other| key(other) == key(item));
    let present = |item: &&T| {
        ours.iter()
            .chain(theirs)
            .any(|other| key(other) == key(item))
    };
    for item in base.unwrap_or_default().iter().filter(present) {
        let current = ours
            .iter()
            .chain(theirs)
            .find(|other| key(other) == key(item))
            .expect("present");
        result.push(current.clone());
    }
    for item in ours.iter().chain(theirs) {
        if !has(&result, item) {
            result.push(item.clone());
        }
    }
    result
}

/// The records of both, each record both hold joined, in `ours`' order then `theirs`' new
/// ones, which arrive.
fn join<T: Clone + Arrive, K: PartialEq>(
    ours: &[T],
    theirs: &[T],
    key: impl Fn(&T) -> K,
    both: impl Fn(&T, &T) -> T,
) -> Vec<T> {
    let mut result: Vec<T> = ours
        .iter()
        .map(
            |item| match theirs.iter().find(|other| key(other) == key(item)) {
                Some(other) => both(item, other),
                None => item.clone(),
            },
        )
        .collect();
    for item in theirs {
        if !ours.iter().any(|other| key(other) == key(item)) {
            let mut arriving = item.clone();
            arriving.arrive();
            result.push(arriving);
        }
    }
    result
}

fn join_block(ours: &Block, theirs: &Block) -> Block {
    let mut block = ours.clone();
    for version in &theirs.versions {
        if !block.versions.iter().any(|v| v.key == version.key) {
            let mut arriving = version.clone();
            arriving.arrived = Some(Arrival::Pending);
            block.versions.push(arriving);
        }
    }
    for response in &theirs.responses {
        if !block.responses.iter().any(|r| r.key == response.key) {
            let mut arriving = response.clone();
            arriving.arrived = Some(Arrival::Pending);
            block.responses.push(arriving);
        }
    }
    block
}

fn join_submission(ours: &Submission, theirs: &Submission) -> Submission {
    let mut submission = ours.clone();
    for change in &theirs.changes {
        let same = |other: &Change| other.what == change.what && other.act == change.act;
        if !submission.changes.iter().any(same) {
            submission.changes.push(Change {
                arrived: Some(Arrival::Pending),
                ..change.clone()
            });
        }
    }
    submission
}

/// A record that a merge brings into this copy, with all it holds.
trait Arrive {
    fn arrive(&mut self);
}

impl Arrive for Block {
    fn arrive(&mut self) {
        for version in &mut self.versions {
            version.arrived = Some(Arrival::Pending);
        }
        for response in &mut self.responses {
            response.arrived = Some(Arrival::Pending);
        }
    }
}

impl Arrive for Submission {
    fn arrive(&mut self) {
        for change in &mut self.changes {
            change.arrived = Some(Arrival::Pending);
        }
    }
}

/// Which of two copies of a session's file a merge takes whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    Ours,
    Theirs,
}

/// Takes one of two copies of a file one session writes alone, its tokens or its activity,
/// named as in Yard Log's folder: `theirs` where its last event ended later, else `ours`.
///
/// # Errors
///
/// A copy is not a mapping, or names its last event's end other than as a time.
pub fn take_session_file(
    name: &str,
    ours: &[u8],
    theirs: &[u8],
) -> Result<Taken, (&'static str, FileError)> {
    let ours = session::last_ended(name, ours).map_err(|error| ("ours", error))?;
    let theirs = session::last_ended(name, theirs).map_err(|error| ("theirs", error))?;
    Ok(if theirs > ours {
        Taken::Theirs
    } else {
        Taken::Ours
    })
}

/// Merges two copies of one file of Yard Log, named as in its folder, over their base, and
/// gives the text of the result.
///
/// # Errors
///
/// A copy does not read as `yardlog`.
pub fn merge_file(
    name: &str,
    base: Option<&[u8]>,
    ours: &[u8],
    theirs: &[u8],
) -> Result<String, (&'static str, FileError)> {
    if name == ROOT {
        let read = |which, bytes: &[u8]| crate::codec::read_root(bytes).map_err(|e| (which, e));
        let base = base.map(|bytes| read("base", bytes)).transpose()?;
        let merged = merge_root(
            base.as_ref(),
            &read("ours", ours)?,
            &read("theirs", theirs)?,
        );
        return Ok(crate::codec::write_root(&merged));
    }
    let read = |which, bytes: &[u8]| crate::codec::read_stage(name, bytes).map_err(|e| (which, e));
    let base = base.map(|bytes| read("base", bytes)).transpose()?;
    let merged = merge_stage(
        base.as_ref(),
        &read("ours", ours)?,
        &read("theirs", theirs)?,
    );
    Ok(crate::codec::write_stage(&merged))
}

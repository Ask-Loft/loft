//! The merge engine (CMP-10): a three-way merge of one file of a `spec/` tree, over keys and
//! ordered lists, that writes a typed conflict record where the two sides disagree
//! (F-32..F-34). It takes only the base, ours and theirs versions of one file (CON-12).

mod conflict;
mod exist;
mod flat;
mod kind;
mod labels;
mod list;
pub mod order;
mod place;
mod record;
mod same;
mod tree;

use std::collections::BTreeMap;
use std::fmt;

use crate::codec::schema::{Kind, field};
use crate::codec::{
    Content, Mode, Problem, ReadError, Rec, Role, Site, SpecPath, Val, keyed_records, read_file,
    write_content,
};
use crate::identity::{Key, Label};
use crate::yaml::{Location, Position};

/// One of the three versions of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Base,
    Ours,
    Theirs,
}

impl fmt::Display for Input {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Base => "base",
            Self::Ours => "ours",
            Self::Theirs => "theirs",
        })
    }
}

/// The merged file, in canonical layout, how many conflict records it holds, and for
/// `spec/labels.yaml` the labels that two publishes gave apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    pub bytes: Vec<u8>,
    pub conflicts: usize,
    pub disputed: Vec<Label>,
}

/// Why a file could not be merged; nothing is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// The path names no file of the Format.
    NotAFormatFile,
    /// Each input that does not read as a file of the Format, or that carries a key twice, with
    /// where (RQ-151).
    Unreadable(Vec<(Input, ReadError)>),
    /// Both sides changed `spec/lint.yaml` to different contents, which no record can hold.
    BothChanged,
    /// The merged file does not read back, or would carry a key twice (RQ-4); the merge writes
    /// nothing rather than a file the reader refuses.
    Unwritable(ReadError),
    /// A record the merge surely keeps that no list of the merged file can take, which only a
    /// side that changed the kind of the record or of its parent can leave; the merge writes
    /// nothing rather than lose it.
    Unplaced(Key),
}

/// Merges one file of a `spec/` tree. An empty base is an absent one, as git gives when both
/// sides added the file. A record file and a registry merge by key: every keyed record merged on
/// its own, with its place as one merged value, and each list rebuilt in merged order.
/// `spec/labels.yaml` merges its counters and bindings on their own terms, and `spec/lint.yaml`
/// takes one side's change.
///
/// # Errors
///
/// The path names no file of the Format, an input does not read, both sides changed the lint
/// configuration apart, or the result would not read back; and a record no list of the merged
/// file can take, which only a key whose kind differs between the versions can leave.
pub fn merge_file(
    path: &SpecPath,
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
) -> Result<Merged, MergeError> {
    let role = Role::of(path).ok_or(MergeError::NotAFormatFile)?;
    let (base, ours, theirs) = read_inputs(path, &role, [base, ours, theirs])?;
    let (content, disputed) = merge_content(base, ours, theirs)?;
    let bytes = write_content(path, role, content);
    let (_, written) = read_file(path, &bytes, Mode::Strict).map_err(MergeError::Unwritable)?;
    if let Some(error) = twice_in(path, &written).into_iter().next() {
        return Err(MergeError::Unwritable(error));
    }
    Ok(Merged {
        conflicts: written.records().map(count).sum(),
        bytes,
        disputed,
    })
}

/// The three inputs, read; an empty base is none, except for the lint configuration.
fn read_inputs(
    path: &SpecPath,
    role: &Role,
    inputs: [&[u8]; 3],
) -> Result<(Option<Content>, Content, Content), MergeError> {
    let mut errors = Vec::new();
    let mut read = |input: Input, bytes: &[u8]| {
        if input == Input::Base && bytes.is_empty() && *role != Role::Lint {
            return None;
        }
        let read = read_file(path, bytes, Mode::Strict);
        match read.map(|(_, content)| (twice_in(path, &content), content)) {
            Ok((twice, content)) if twice.is_empty() => Some(content),
            Ok((twice, _)) => {
                errors.extend(twice.into_iter().map(|error| (input, error)));
                None
            }
            Err(error) => {
                errors.push((input, error));
                None
            }
        }
    };
    let [base, ours, theirs] = inputs;
    let base = read(Input::Base, base);
    let ours = read(Input::Ours, ours);
    let theirs = read(Input::Theirs, theirs);
    match (ours, theirs, errors.is_empty()) {
        (Some(ours), Some(theirs), true) => Ok((base, ours, theirs)),
        _ => Err(MergeError::Unreadable(errors)),
    }
}

/// The merged content: the labels file on its own terms, a record file or a registry by key,
/// and the lint configuration as one value.
fn merge_content(
    base: Option<Content>,
    ours: Content,
    theirs: Content,
) -> Result<(Content, Vec<Label>), MergeError> {
    match (base, ours, theirs) {
        (base, Content::Record(ours), Content::Record(theirs)) if ours.kind == Kind::Labels => {
            let base = match base {
                Some(Content::Record(base)) => Some(base),
                _ => None,
            };
            let (record, disputed) = labels::merge(base.as_ref(), &ours, &theirs);
            Ok((Content::Record(record), disputed))
        }
        (base, ours @ Content::Record(_), theirs @ Content::Record(_))
        | (base, ours @ Content::Registry(_), theirs @ Content::Registry(_)) => {
            let merged =
                tree::merge(base.as_ref(), &ours, &theirs).map_err(MergeError::Unplaced)?;
            Ok((merged, Vec::new()))
        }
        (base, Content::Lint(ours), Content::Lint(theirs)) => {
            let base = match base {
                Some(Content::Lint(base)) => base,
                _ => Vec::new(),
            };
            Ok((Content::Lint(lint(&base, ours, theirs)?), Vec::new()))
        }
        _ => Err(MergeError::NotAFormatFile),
    }
}

/// The lint configuration, which no record holds: one side's change wins, and equal changes
/// coalesce.
fn lint(base: &[u8], ours: Vec<u8>, theirs: Vec<u8>) -> Result<Vec<u8>, MergeError> {
    if ours == theirs || theirs == base {
        Ok(ours)
    } else if ours == base {
        Ok(theirs)
    } else {
        Err(MergeError::BothChanged)
    }
}

/// Each key that two keyed records of a version carry, at the `key` field of the first and of
/// the other: such an input cannot be read as a file of the Format, and such a merged file is
/// not written (RQ-4, RQ-151).
fn twice_in(path: &SpecPath, content: &Content) -> Vec<ReadError> {
    let site = |position: Position| Site {
        path: path.clone(),
        location: Some(Location::At(position)),
    };
    let mut seen = BTreeMap::new();
    let mut errors = Vec::new();
    for record in keyed_records(content.records()) {
        let (Some(key), Some(Val::Scalar { position, .. })) =
            (record.key(), record.get(field::KEY))
        else {
            continue;
        };
        match seen.get(&key) {
            Some(&first) => errors.push(ReadError {
                sites: vec![site(first), site(*position)],
                problem: Problem::DuplicateKey(key),
            }),
            None => {
                seen.insert(key, *position);
            }
        }
    }
    errors
}

/// The conflict records a record and every record inside it hold.
fn count(record: &Rec) -> usize {
    record.conflicts.len() + record.children().map(count).sum::<usize>()
}

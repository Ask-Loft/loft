//! The merge engine (CMP-10): a three-way merge of one file of a `spec/` tree, over keys and
//! ordered lists, that writes a typed conflict record where the two sides disagree
//! (F-32..F-34). It takes only the base, ours and theirs versions of one file (CON-12).

mod conflict;
mod exist;
mod flat;
mod kind;
mod labels;
mod list;
mod moves;
pub mod order;
mod place;
mod places;
mod record;
mod same;
mod tree;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::codec::schema::{Field, Item, Kind, Order, Shape, field};
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
    /// The merged file does not read back; the merge writes nothing rather than a file the
    /// reader refuses.
    Unwritable(ReadError),
    /// The legacy merge's result would carry one key twice, which no tree may (RQ-4); the merge
    /// writes nothing. The merge by key reports such a result as [`MergeError::Unwritable`].
    DuplicateKey(Key),
    /// In the legacy merge, a record moved within the file has no parent left on either side to
    /// hold it; in the merge by key, a record no list of the merged file can take, which only a
    /// key whose kind differs between the versions leaves. The merge writes nothing rather than
    /// lose it.
    Unplaced(Key),
}

/// Merges one file of a `spec/` tree. An empty base is an absent one, as git gives when both
/// sides added the file. A record file and a registry merge by key: every keyed record merged on
/// its own, with its place as one merged value, and each list rebuilt in merged order.
/// `spec/labels.yaml` and `spec/lint.yaml` merge as [`merge_file_legacy`] merges them.
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
    merge(path, [base, ours, theirs], true)
}

/// Merges one file of a `spec/` tree as [`merge_file`] did before it merged record files by
/// key: a move pre-pass, then each record merged within its parent's lists. Kept to compare the
/// two merges until the legacy merge is removed.
///
/// # Errors
///
/// As [`merge_file`]; and a merged file that would carry a key twice, or a record moved out of
/// every parent left on either side.
#[doc(hidden)]
pub fn merge_file_legacy(
    path: &SpecPath,
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
) -> Result<Merged, MergeError> {
    merge(path, [base, ours, theirs], false)
}

/// Merges one file, a record file but `spec/labels.yaml` or a registry by key when `by_key`,
/// and every other file as the legacy merge does.
fn merge(path: &SpecPath, inputs: [&[u8]; 3], by_key: bool) -> Result<Merged, MergeError> {
    let role = Role::of(path).ok_or(MergeError::NotAFormatFile)?;
    let (base, ours, theirs) = read_inputs(path, &role, inputs)?;
    let keyed = match (&ours, &theirs) {
        (Content::Record(top), Content::Record(_)) => top.kind != Kind::Labels,
        (Content::Registry(_), Content::Registry(_)) => true,
        _ => false,
    };
    if by_key && keyed {
        let content = tree::merge(base.as_ref(), &ours, &theirs).map_err(MergeError::Unplaced)?;
        let (bytes, written) = write_back(path, role, content)?;
        if let Some(error) = twice_in(path, &written).into_iter().next() {
            return Err(MergeError::Unwritable(error));
        }
        return Ok(merged(bytes, &written, Vec::new()));
    }
    let (content, disputed) = merge_content(base, ours, theirs)?;
    let (bytes, written) = write_back(path, role, content)?;
    if let Some(key) = carried_twice(&written) {
        return Err(MergeError::DuplicateKey(key));
    }
    Ok(merged(bytes, &written, disputed))
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

/// The merged content in canonical layout, and read back.
fn write_back(
    path: &SpecPath,
    role: Role,
    content: Content,
) -> Result<(Vec<u8>, Content), MergeError> {
    let bytes = write_content(path, role, content);
    let (_, written) = read_file(path, &bytes, Mode::Strict).map_err(MergeError::Unwritable)?;
    Ok((bytes, written))
}

fn merged(bytes: Vec<u8>, written: &Content, disputed: Vec<Label>) -> Merged {
    Merged {
        conflicts: written.records().map(count).sum(),
        bytes,
        disputed,
    }
}

fn merge_content(
    base: Option<Content>,
    ours: Content,
    theirs: Content,
) -> Result<(Content, Vec<Label>), MergeError> {
    Ok(match (base, ours, theirs) {
        (base, Content::Record(mut ours), Content::Record(mut theirs)) => {
            let mut base = match base {
                Some(Content::Record(base)) => Some(base),
                _ => None,
            };
            if ours.kind == Kind::Labels {
                let (record, disputed) = labels::merge(base.as_ref(), &ours, &theirs);
                return Ok((Content::Record(record), disputed));
            }
            moves::normalise(base.as_mut(), &mut ours, &mut theirs)
                .map_err(MergeError::Unplaced)?;
            (
                Content::Record(record::merge_host(base.as_ref(), &ours, &theirs)),
                Vec::new(),
            )
        }
        (base, Content::Registry(ours), Content::Registry(theirs)) => {
            let base = match base {
                Some(Content::Registry(base)) => base,
                _ => Vec::new(),
            };
            (
                Content::Registry(registry(&base, &ours, &theirs)),
                Vec::new(),
            )
        }
        (base, Content::Lint(ours), Content::Lint(theirs)) => {
            let base = match base {
                Some(Content::Lint(base)) => base,
                _ => Vec::new(),
            };
            (Content::Lint(lint(&base, ours, theirs)?), Vec::new())
        }
        _ => return Err(MergeError::NotAFormatFile),
    })
}

/// A registry's entries, merged by key; each entry holds its own conflicts.
fn registry(base: &[Rec], ours: &[Rec], theirs: &[Rec]) -> Vec<Rec> {
    let first = ours
        .first()
        .or_else(|| theirs.first())
        .or_else(|| base.first());
    let Some(kind) = first.map(|entry| entry.kind) else {
        return Vec::new();
    };
    let entries = Field {
        name: "entries",
        required: false,
        shape: Shape::List(Item::Record(kind)),
        order: Order::ByKey,
        supply: None,
    };
    let values =
        |entries: &[Rec]| -> Vec<Val> { entries.iter().cloned().map(Val::Record).collect() };
    // Entries hold their own conflicts, and a list merged by key has no place to conflict over.
    let mut none = Vec::new();
    let mut host = conflict::Host::root(&mut none);
    let merged = list::merge_list(
        &entries,
        [&values(base), &values(ours), &values(theirs)],
        &mut host,
    );
    list::records(merged)
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

/// Each key that two keyed records of an input carry, at the `key` field of the first and of
/// the other: the input cannot be read as a file of the Format (RQ-4, RQ-151).
fn twice_in(path: &SpecPath, content: &Content) -> Vec<ReadError> {
    let site = |position: Position| Site {
        path: path.clone(),
        location: Some(Location::At(position)),
    };
    let mut seen = BTreeMap::new();
    let mut errors = Vec::new();
    for record in keyed_records(content.records()) {
        let Some(Val::Scalar { text, position }) = record.get(field::KEY) else {
            continue;
        };
        let Ok(key) = Key::parse(text) else {
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

/// A key that two keyed records of the merged file carry, which the merge refuses to write
/// (RQ-4).
fn carried_twice(content: &Content) -> Option<Key> {
    let mut seen = BTreeSet::new();
    keyed_records(content.records())
        .into_iter()
        .filter_map(|record| Key::parse(record.text(field::KEY)?).ok())
        .find(|key| !seen.insert(*key))
}

/// The conflict records a record and every record inside it hold.
fn count(record: &Rec) -> usize {
    record.conflicts.len() + record.children().map(count).sum::<usize>()
}

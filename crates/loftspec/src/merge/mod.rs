//! The merge engine (CMP-10): a three-way merge of one file of a `spec/` tree, over keys and
//! ordered lists, that writes a typed conflict record where the two sides disagree
//! (F-32..F-34). It takes only the base, ours and theirs versions of one file (CON-12).

mod conflict;
mod labels;
mod list;
mod moves;
pub mod order;
mod places;
mod record;
mod same;

use std::collections::BTreeSet;
use std::fmt;

use crate::codec::schema::{Field, Item, Kind, Order, Shape, field};
use crate::codec::{
    Content, Mode, ReadError, Rec, Role, SpecPath, Val, keyed, read_file, write_content,
};
use crate::identity::{Key, Label};

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
    /// Each input that does not read as a file of the Format, with where (RQ-151).
    Unreadable(Vec<(Input, ReadError)>),
    /// Both sides changed `spec/lint.yaml` to different contents, which no record can hold.
    BothChanged,
    /// The merged file does not read back; the merge writes nothing rather than a file the
    /// reader refuses.
    Unwritable(ReadError),
    /// The merged file would carry one key twice, which no tree may (RQ-4); the merge writes
    /// nothing.
    DuplicateKey(Key),
    /// A record moved within the file has no parent left on either side to hold it; the merge
    /// writes nothing rather than lose it.
    Unplaced(Key),
}

/// Merges one file of a `spec/` tree. An empty base is an absent one, as git gives when both
/// sides added the file.
///
/// # Errors
///
/// The path names no file of the Format, an input does not read, both sides changed the lint
/// configuration apart, or the result would not read back.
pub fn merge_file(
    path: &SpecPath,
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
) -> Result<Merged, MergeError> {
    let role = Role::of(path).ok_or(MergeError::NotAFormatFile)?;
    let mut errors = Vec::new();
    let mut read = |input: Input, bytes: &[u8]| {
        if input == Input::Base && bytes.is_empty() && role != Role::Lint {
            return None;
        }
        match read_file(path, bytes, Mode::Strict) {
            Ok((_, content)) => Some(content),
            Err(error) => {
                errors.push((input, error));
                None
            }
        }
    };
    let base = read(Input::Base, base);
    let ours = read(Input::Ours, ours);
    let theirs = read(Input::Theirs, theirs);
    let (Some(ours), Some(theirs), true) = (ours, theirs, errors.is_empty()) else {
        return Err(MergeError::Unreadable(errors));
    };
    let (content, disputed) = merge_content(base, ours, theirs)?;
    let bytes = write_content(path, role, content);
    let (_, written) = read_file(path, &bytes, Mode::Strict).map_err(MergeError::Unwritable)?;
    if let Some(key) = carried_twice(&written) {
        return Err(MergeError::DuplicateKey(key));
    }
    Ok(Merged {
        conflicts: written.records().map(count).sum(),
        bytes,
        disputed,
    })
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

/// A key that two keyed records of the merged file carry, which the merge refuses to write
/// (RQ-4).
fn carried_twice(content: &Content) -> Option<Key> {
    let mut found = Vec::new();
    content
        .records()
        .for_each(|record| keyed(record, &mut found));
    let mut seen = BTreeSet::new();
    found
        .into_iter()
        .filter_map(|record| Key::parse(record.text(field::KEY)?).ok())
        .find(|key| !seen.insert(*key))
}

/// The conflict records a record and every record inside it hold.
fn count(record: &Rec) -> usize {
    record.conflicts.len() + record.children().map(count).sum::<usize>()
}

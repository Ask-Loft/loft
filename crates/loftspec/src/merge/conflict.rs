//! The conflicts a merge writes: each on the record that holds the field, named by its path
//! from that record (F-34), and each written once (`hold`).

use super::order::{Anchor, PositionConflict};
use super::same::holds;
use crate::codec::schema::field;
use crate::codec::{ConflictRec, Rec, Val};
use crate::identity::Key;

/// Where the conflicts of the record being merged go, and the path from that record to the
/// unkeyed record whose fields are being merged.
pub(super) struct Host<'a> {
    pub(super) prefix: String,
    pub(super) conflicts: &'a mut Vec<ConflictRec>,
}

impl<'a> Host<'a> {
    /// The host for the record being merged itself, whose conflicts go to `conflicts`.
    pub(super) const fn root(conflicts: &'a mut Vec<ConflictRec>) -> Self {
        Host {
            prefix: String::new(),
            conflicts,
        }
    }

    /// The path of a field of the record being merged.
    pub(super) fn path(&self, name: &str) -> String {
        format!("{}{name}", self.prefix)
    }

    /// The host for the unkeyed record in field `name`, whose conflicts the same record holds.
    pub(super) fn nest(&mut self, name: &str) -> Host<'_> {
        Host {
            prefix: format!("{}{name}.", self.prefix),
            conflicts: self.conflicts,
        }
    }

    /// Records a conflict over field `name` with each side's value: base, ours, theirs.
    pub(super) fn conflict(&mut self, name: &str, sides: [Option<&Val>; 3]) {
        let path = self.path(name);
        self.conflicts.push(conflict(&path, sides));
    }
}

/// A conflict record over `path`; a side whose value is absent omits its member (AC-859).
pub(super) fn conflict(path: &str, sides: [Option<&Val>; 3]) -> ConflictRec {
    ConflictRec::new(path, sides.map(|side| super::same::present(side).cloned()))
}

/// One of the two sides that changed the base: ours or theirs.
#[derive(Debug, Clone, Copy)]
pub(super) enum Side {
    Ours,
    Theirs,
}

impl Side {
    /// The other side.
    pub(super) const fn other(self) -> Self {
        match self {
            Self::Ours => Self::Theirs,
            Self::Theirs => Self::Ours,
        }
    }
}

/// A conflict over `record` on a keyed record that `deleter` deleted and the other side kept:
/// each side names the record's key, and the deleting side `null` (AC-473).
pub(super) fn record_conflict(record: &Rec, deleter: Side) -> ConflictRec {
    let key = record.get(field::KEY);
    let null = Val::Null {
        position: record.position,
    };
    let sides = match deleter {
        Side::Ours => [key, Some(&null), key],
        Side::Theirs => [key, key, Some(&null)],
    };
    conflict(field::RECORD, sides)
}

/// A conflict over `parent` on a record the sides put at different parents, or that the merge
/// put elsewhere than either side: each side names the record's parent there, or has none
/// where it lacks the record.
pub(super) fn parent_conflict(sides: [Option<Val>; 3]) -> ConflictRec {
    ConflictRec::new(field::PARENT, sides)
}

/// A conflict over where a record goes: each side names the record it follows, by its key, or
/// `first`.
pub(super) fn position_conflict(position: &PositionConflict<Key>) -> ConflictRec {
    let anchor = |anchor: &Option<Anchor<Key>>| {
        anchor.as_ref().map(|anchor| match anchor {
            Anchor::First => Val::scalar("first"),
            Anchor::After(key) => Val::scalar(key),
        })
    };
    let sides = [&position.base, &position.ours, &position.theirs].map(anchor);
    ConflictRec::new(field::POSITION, sides)
}

/// Gives a record a conflict, unless it holds that conflict already.
pub(super) fn hold(record: &mut Rec, conflict: ConflictRec) {
    if !holds(record, &conflict) {
        record.conflicts.push(conflict);
    }
}

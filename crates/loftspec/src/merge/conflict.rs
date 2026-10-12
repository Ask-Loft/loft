//! The conflicts a merge writes: each on the record that holds the field, named by its path
//! from that record (D-11 F-34).

use crate::codec::schema::{Kind, field};
use crate::codec::{Rec, Val};

/// Where the conflicts of the record being merged go, and the path from that record to the
/// unkeyed record whose fields are being merged.
pub(super) struct Host<'a> {
    pub(super) prefix: String,
    pub(super) conflicts: &'a mut Vec<Val>,
}

impl<'a> Host<'a> {
    /// The host for the record being merged itself, whose conflicts go to `conflicts`.
    pub(super) const fn root(conflicts: &'a mut Vec<Val>) -> Self {
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
pub(super) fn conflict(path: &str, sides: [Option<&Val>; 3]) -> Val {
    let mut record = Rec::new(Kind::Conflict);
    record.set_text(field::FIELD, path);
    for (name, side) in [field::BASE, field::OURS, field::THEIRS]
        .into_iter()
        .zip(sides)
    {
        if let Some(value) = super::same::present(side) {
            record.set(name, value.clone());
        }
    }
    Val::Record(record)
}

/// Adds a conflict to the `conflicts` list of a record that holds it.
pub(super) fn push(record: &mut Rec, conflict: Val) {
    record.list_mut(field::CONFLICTS).push(conflict);
}

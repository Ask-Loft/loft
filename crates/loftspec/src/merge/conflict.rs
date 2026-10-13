//! The conflicts a merge writes: each on the record that holds the field, named by its path
//! from that record (D-11 F-34).

use crate::codec::{ConflictRec, Val};

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

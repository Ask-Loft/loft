//! A field of a record named by its path from the record, dotted through the unkeyed records
//! inside it, as a conflict names it (`statement.shall`).

use super::record::{Rec, Val};

impl Rec {
    /// The value of the field at `path`; none where the path leads to no value.
    #[must_use]
    pub(crate) fn at_path(&self, path: &str) -> Option<&Val> {
        let (head, rest) = split(path);
        self.kind.field(head)?;
        let value = self.get(head)?;
        match (rest, value) {
            (None, value) => Some(value),
            (Some(rest), Val::Record(inner)) => inner.at_path(rest),
            _ => None,
        }
    }

    /// Sets the field at `path` to `value`, or leaves it out for none; whether the path names a
    /// field of this record or of an unkeyed record it holds.
    pub(crate) fn set_at_path(&mut self, path: &str, value: Option<Val>) -> bool {
        let (head, rest) = split(path);
        let Some((_, field)) = self.kind.field(head) else {
            return false;
        };
        let name = field.name;
        match (rest, value) {
            (None, Some(value)) => self.set(name, value),
            (None, None) => self.fields.retain(|(each, _)| *each != name),
            (Some(rest), value) => {
                return match self.get_mut(name) {
                    Some(Val::Record(inner)) => inner.set_at_path(rest, value),
                    _ => false,
                };
            }
        }
        true
    }

    /// The record that holds the field at `path`: this record, or the unkeyed record inside it
    /// that the path leads through.
    #[must_use]
    pub(crate) fn holder_of(&self, path: &str) -> Option<&Self> {
        match holder_path(path) {
            "" => Some(self),
            parent => match self.at_path(parent)? {
                Val::Record(inner) => Some(inner),
                _ => None,
            },
        }
    }
}

/// The path of the record that holds the field at `path`; empty for the record itself.
pub(crate) fn holder_path(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(parent, _)| parent)
}

/// The first segment of a path, and the rest after its dot.
fn split(path: &str) -> (&str, Option<&str>) {
    match path.split_once('.') {
        Some((head, rest)) => (head, Some(rest)),
        None => (path, None),
    }
}

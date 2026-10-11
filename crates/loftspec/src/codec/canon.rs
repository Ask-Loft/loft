//! The orders the canonical layout fixes (RQ-36, AC-807, AC-808, AC-790): edge targets by key,
//! subjects by role then target, bindings by key, registry entries by name then key.

use super::files::{Content, Tree};
use super::record::{Rec, Val};
use super::schema::Kind;

/// Sorts every list the canonical layout orders, throughout the tree.
pub(crate) fn canonicalize(tree: &mut Tree) {
    for (_, content) in tree.files.values_mut() {
        match content {
            Content::Record(record) => sort(record),
            Content::Registry(entries) => {
                entries.iter_mut().for_each(sort);
                entries.sort_by(|left, right| entry_order(left).cmp(&entry_order(right)));
            }
            Content::Lint(_) => {}
        }
    }
}

fn entry_order(entry: &Rec) -> (&str, &str) {
    (
        entry.text("name").unwrap_or_default(),
        entry.text("key").unwrap_or_default(),
    )
}

/// The text a value sorts by: a scalar's text, a former label's label, or a record's key.
fn sort_text(value: &Val) -> &str {
    match value {
        Val::Scalar { text, .. } => text,
        Val::Record(record) => record
            .text("formerLabel")
            .or_else(|| record.text("key"))
            .unwrap_or_default(),
        Val::List(_) => "",
    }
}

fn sort(record: &mut Rec) {
    let kind = record.kind;
    for (name, value) in &mut record.fields {
        value.records_mut().for_each(sort);
        let Val::List(items) = value else {
            continue;
        };
        match (*name, kind) {
            ("realizes" | "scopedTo" | "mentions", _) | ("bindings", Kind::Labels) => {
                items.sort_by(|left, right| sort_text(left).cmp(sort_text(right)));
            }
            ("subjects", Kind::Decision) => {
                items.sort_by(|left, right| subject_order(left).cmp(&subject_order(right)));
            }
            _ => {}
        }
    }
}

fn subject_order(subject: &Val) -> (&str, &str) {
    match subject {
        Val::Record(record) => (
            record.text("role").unwrap_or_default(),
            record.get("node").map_or("", sort_text),
        ),
        _ => ("", ""),
    }
}

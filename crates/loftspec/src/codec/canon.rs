//! The orders the canonical layout fixes (RQ-36): each list field as its schema `Order` says
//! (AC-807, AC-808), and registry entries by name, then key (AC-790).

use super::files::{Content, Tree};
use super::record::{Rec, Val};
use super::schema::Order;
use super::schema::field;

/// Sorts every list the canonical layout orders, throughout the tree.
pub(crate) fn canonicalize(tree: &mut Tree) {
    for (_, content) in tree.files.values_mut() {
        content.records_mut().for_each(sort);
        if let Content::Registry(entries) = content {
            entries.sort_by(|left, right| entry_order(left).cmp(&entry_order(right)));
        }
    }
}

fn entry_order(entry: &Rec) -> (&str, &str) {
    (
        entry.text(field::NAME).unwrap_or_default(),
        entry.text(field::KEY).unwrap_or_default(),
    )
}

/// The text an entry sorts by: a key's text, a former label's label, or a binding's key.
fn sort_text(value: &Val) -> &str {
    match value {
        Val::Scalar { text, .. } => text,
        Val::Record(record) => record
            .fields
            .first()
            .and_then(|(_, first)| first.text())
            .unwrap_or_default(),
        Val::List(_) | Val::Null { .. } | Val::Ext { .. } => "",
    }
}

/// A subject's role, then its node.
fn subject_order(subject: &Val) -> (&str, &str) {
    match subject {
        Val::Record(record) => (
            record.text(field::ROLE).unwrap_or_default(),
            record.get(field::NODE).map_or("", sort_text),
        ),
        _ => ("", ""),
    }
}

fn sort(record: &mut Rec) {
    let kind = record.kind;
    for (name, value) in &mut record.fields {
        value.records_mut().for_each(sort);
        let Val::List(items) = value else {
            continue;
        };
        let (_, field) = kind.field(name).expect("a field the schema defines");
        match field.order {
            Order::Authored => {}
            Order::ByKey | Order::ByText => {
                items.sort_by(|left, right| sort_text(left).cmp(sort_text(right)));
            }
            Order::ByRoleThenTarget => {
                items.sort_by(|left, right| subject_order(left).cmp(&subject_order(right)));
            }
        }
    }
}

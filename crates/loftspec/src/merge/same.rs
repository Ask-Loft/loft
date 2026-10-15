//! Equality of field values as the merge sees it: by content alone, never by where a value
//! sat in its file, with an empty list the same as an absent one (AC-799).

use crate::codec::{ConflictRec, Rec, Val};

/// Whether two sides hold the same value of a field.
pub(super) fn same(left: Option<&Val>, right: Option<&Val>) -> bool {
    match (present(left), present(right)) {
        (None, None) => true,
        (Some(left), Some(right)) => same_value(left, right),
        _ => false,
    }
}

/// A value that is there: an empty list counts as absent.
pub(super) fn present(value: Option<&Val>) -> Option<&Val> {
    value.filter(|value| !matches!(value, Val::List(items) if items.is_empty()))
}

pub(super) fn same_value(left: &Val, right: &Val) -> bool {
    match (left, right) {
        (Val::Scalar { text: left, .. }, Val::Scalar { text: right, .. }) => left == right,
        (Val::Null { .. }, Val::Null { .. }) => true,
        (Val::List(left), Val::List(right)) => all_same(left, right, same_value),
        (Val::Record(left), Val::Record(right)) => same_record(left, right),
        _ => false,
    }
}

/// Whether two records of one kind hold the same present fields with the same values, and the
/// same conflicts: a record whose conflicts differ is not the same record.
pub(super) fn same_record(left: &Rec, right: &Rec) -> bool {
    let (left_fields, right_fields) = (present_fields(left), present_fields(right));
    left.kind == right.kind
        && left_fields.len() == right_fields.len()
        && left_fields
            .iter()
            .zip(&right_fields)
            .all(|((left_name, left), (right_name, right))| {
                left_name == right_name && same_value(left, right)
            })
        && all_same(&left.conflicts, &right.conflicts, same_conflict)
}

/// Whether two conflicts name the same field with the same sides.
pub(super) fn same_conflict(left: &ConflictRec, right: &ConflictRec) -> bool {
    left.field == right.field
        && left
            .sides()
            .into_iter()
            .zip(right.sides())
            .all(|((_, left), (_, right))| same(left, right))
}

/// Whether a record holds a conflict the same as `conflict`.
pub(super) fn holds(record: &Rec, conflict: &ConflictRec) -> bool {
    record
        .conflicts
        .iter()
        .any(|held| same_conflict(held, conflict))
}

/// Whether two lists hold the same items in the same order, by `same`.
fn all_same<T>(left: &[T], right: &[T], same: fn(&T, &T) -> bool) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| same(left, right))
}

/// The fields of a record that hold a present value.
fn present_fields(record: &Rec) -> Vec<&(&'static str, Val)> {
    record
        .fields
        .iter()
        .filter(|(_, value)| present(Some(value)).is_some())
        .collect()
}

/// A value as text that two values share exactly when they are the same.
pub(super) fn text(value: &Val) -> String {
    match value {
        Val::Scalar { text, .. } => text.clone(),
        Val::Null { .. } => "null".to_owned(),
        Val::Ext { value, .. } => format!("ext{}", crate::ext::to_json(value)),
        Val::List(items) => format!("[{}]", items.iter().map(text).collect::<Vec<_>>().join(",")),
        Val::Record(record) => {
            let fields = record
                .fields
                .iter()
                .filter(|(_, value)| present(Some(value)).is_some())
                .map(|(name, value)| format!("{name}={}", text(value)));
            let mut text = format!("{{{}}}", fields.collect::<Vec<_>>().join(";"));
            if !record.conflicts.is_empty() {
                let conflicts: Vec<String> = record.conflicts.iter().map(conflict_text).collect();
                text.push('[');
                text.push_str(&conflicts.join(","));
                text.push(']');
            }
            text
        }
    }
}

/// A conflict as text, its field and each present side by name, as a record's text names its
/// fields.
fn conflict_text(conflict: &ConflictRec) -> String {
    let sides = conflict
        .sides()
        .into_iter()
        .filter_map(|(name, side)| present(side).map(|side| format!("{name}={}", text(side))));
    let fields: Vec<String> = std::iter::once(format!("field={}", conflict.field))
        .chain(sides)
        .collect();
    format!("{{{}}}", fields.join(";"))
}

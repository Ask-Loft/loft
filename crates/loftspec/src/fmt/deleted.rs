//! The labels of deleted nodes (Labels and aliases): the binding of each key no node or decision
//! carries is dropped, the counters left as they are so that its labels are never allocated
//! again, and a retired or superseded decision's reference to such a node takes the node's
//! former label, the live label its binding held. A tree without such a binding costs one scan.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::schema::{Item, Kind, field};
use crate::codec::{Content, Rec, Role, Tree, Val, keyed};
use crate::decision::StateKind;
use crate::identity::{Key, Label};

/// Settles the bindings of the nodes and decisions the tree no longer holds; the label and key
/// of each binding dropped, in key order.
pub(super) fn settle(tree: &mut Tree) -> Vec<(Label, Key)> {
    let carried = carried(tree);
    let gone = drop_bindings(tree, &carried);
    if gone.is_empty() {
        return Vec::new();
    }
    for (_, content) in tree.files.values_mut() {
        for record in content.records_mut() {
            former_labels(record, &gone, false);
        }
    }
    // The reader checked each binding's label and key.
    gone.iter()
        .filter_map(|(key, label)| Some((Label::parse(label).ok()?, Key::parse(key).ok()?)))
        .collect()
}

/// The keys of every node and decision of the tree.
fn carried(tree: &Tree) -> BTreeSet<String> {
    let mut records = Vec::new();
    for (_, content) in tree.files.values() {
        for record in content.records() {
            keyed(record, &mut records);
        }
    }
    records
        .into_iter()
        .filter(|record| record.kind == Kind::Decision || record.kind.node_kind().is_some())
        .filter_map(|record| record.text(field::KEY))
        .map(str::to_owned)
        .collect()
}

/// Drops each binding whose key `carried` lacks; the live label of each, by its key.
fn drop_bindings(tree: &mut Tree, carried: &BTreeSet<String>) -> BTreeMap<String, String> {
    let mut gone = BTreeMap::new();
    for (role, content) in tree.files.values_mut() {
        let (Role::Labels, Content::Record(labels)) = (role, content) else {
            continue;
        };
        let Some(Val::List(bindings)) = labels.get_mut(field::BINDINGS) else {
            continue;
        };
        bindings.retain(|binding| {
            let Val::Record(binding) = binding else {
                return true;
            };
            match (binding.text(field::KEY), binding.text(field::LABEL)) {
                (Some(key), Some(label)) if !carried.contains(key) => {
                    gone.insert(key.to_owned(), label.to_owned());
                    false
                }
                _ => true,
            }
        });
    }
    gone
}

/// Whether a decision is retired or superseded.
fn settled(decision: &Rec) -> bool {
    let kind = decision
        .record(field::STATE)
        .and_then(|state| state.text(field::KIND))
        .and_then(StateKind::parse);
    matches!(kind, Some(StateKind::Retired | StateKind::Superseded))
}

/// Writes each node reference to a key of `gone` in a record and everything in it as the
/// node's former label, where the record is, or sits in, a retired or superseded decision.
fn former_labels(record: &mut Rec, gone: &BTreeMap<String, String>, in_settled: bool) {
    let in_settled = in_settled || (record.kind == Kind::Decision && settled(record));
    let kind = record.kind;
    for (name, value) in &mut record.fields {
        let node_ref = in_settled
            && kind
                .field(name)
                .is_some_and(|(_, field)| field.shape.item() == Item::NodeRef);
        let items = match value {
            Val::List(items) => items.as_mut_slice(),
            single => std::slice::from_mut(single),
        };
        for item in items {
            if let Val::Record(child) = item {
                former_labels(child, gone, in_settled);
            } else if node_ref && let Some(label) = item.text().and_then(|key| gone.get(key)) {
                *item = former_label(label);
            }
        }
    }
}

/// `{formerLabel: <label>}` (AC-809).
fn former_label(label: &str) -> Val {
    let mut record = Rec::new(Kind::FormerLabel);
    record.set_text(field::FORMER_LABEL, label);
    Val::Record(record)
}

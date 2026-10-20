//! The labels of nodes that leave and come back (Labels and aliases): the binding of each key no
//! node or decision carries is marked departed, its labels and the counters left as they are so
//! that its labels are never given to another key; a retired or superseded decision's reference
//! to such a node takes the node's former label, the live label its binding held; and a node
//! that comes back is named by its key again wherever its former label stood. A tree without
//! such a binding costs one scan.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::schema::{Item, Kind, field};
use crate::codec::{Content, Rec, Role, Tree, Val, keyed_records};
use crate::decision::StateKind;
use crate::identity::{Key, Label};

/// The departed bindings of a tree: the live label of each key no element carries, and the key
/// of each departed label whose key an element carries again.
#[derive(Default)]
struct Departed {
    gone: BTreeMap<String, String>,
    returned: BTreeMap<String, String>,
}

/// Settles the bindings of the nodes and decisions the tree no longer holds, or holds again; the
/// label and key of each binding newly marked departed, in key order.
pub(super) fn settle(tree: &mut Tree) -> Vec<(Label, Key)> {
    let carried = carried(tree);
    let (departed, newly) = depart(tree, &carried);
    if departed.gone.is_empty() && departed.returned.is_empty() {
        return Vec::new();
    }
    for (_, content) in tree.files.values_mut() {
        for record in content.records_mut() {
            former_labels(record, &departed.gone, false);
            returned_keys(record, &departed.returned);
        }
    }
    // The reader checked each binding's label and key.
    newly
        .iter()
        .filter_map(|(key, label)| Some((Label::parse(label).ok()?, Key::parse(key).ok()?)))
        .collect()
}

/// The keys of every node and decision of the tree.
fn carried(tree: &Tree) -> BTreeSet<String> {
    let files = tree.files.values();
    keyed_records(files.flat_map(|(_, content)| content.records()))
        .into_iter()
        .filter(|record| record.kind == Kind::Decision || record.kind.node_kind().is_some())
        .filter_map(|record| record.text(field::KEY))
        .map(str::to_owned)
        .collect()
}

/// Marks departed each binding whose key `carried` lacks, removing none: the departed bindings
/// then, and the live label of each binding it marked, by its key.
fn depart(tree: &mut Tree, carried: &BTreeSet<String>) -> (Departed, BTreeMap<String, String>) {
    let mut departed = Departed::default();
    let mut newly = BTreeMap::new();
    for binding in bindings(tree) {
        let (Some(key), Some(label)) = (binding.text(field::KEY), binding.text(field::LABEL))
        else {
            continue;
        };
        let (key, label) = (key.to_owned(), label.to_owned());
        let marked = binding.get(field::DEPARTED).is_some();
        if carried.contains(&key) {
            if marked {
                departed.returned.insert(label, key);
            }
            continue;
        }
        if !marked {
            binding.set_text(field::DEPARTED, "true");
            newly.insert(key.clone(), label.clone());
        }
        departed.gone.insert(key, label);
    }
    (departed, newly)
}

/// The binding records of the bindings file.
fn bindings(tree: &mut Tree) -> impl Iterator<Item = &mut Rec> {
    let labels = tree
        .files
        .values_mut()
        .find_map(|(role, content)| match (role, content) {
            (Role::Labels, Content::Record(labels)) => Some(labels),
            _ => None,
        });
    let list = labels.and_then(|labels| match labels.get_mut(field::BINDINGS) {
        Some(Val::List(bindings)) => Some(bindings),
        _ => None,
    });
    list.into_iter()
        .flatten()
        .filter_map(|binding| match binding {
            Val::Record(binding) => Some(binding),
            _ => None,
        })
}

/// Whether a decision is retired or superseded.
fn settled(decision: &Rec) -> bool {
    let kind = decision
        .record(field::STATE)
        .and_then(|state| state.text(field::KIND))
        .and_then(StateKind::parse);
    matches!(kind, Some(StateKind::Retired | StateKind::Superseded))
}

/// The items of each node reference field of a record, and each other record the record holds.
fn node_refs(record: &mut Rec) -> (Vec<&mut Val>, Vec<&mut Rec>) {
    let kind = record.kind;
    let mut references = Vec::new();
    let mut children = Vec::new();
    for (name, value) in &mut record.fields {
        let node_ref = kind
            .field(name)
            .is_some_and(|(_, field)| field.shape.item() == Item::NodeRef);
        let items = match value {
            Val::List(items) => items.as_mut_slice(),
            single => std::slice::from_mut(single),
        };
        for item in items {
            if node_ref {
                references.push(item);
            } else if let Val::Record(child) = item {
                children.push(child);
            }
        }
    }
    (references, children)
}

/// Writes each node reference to a key of `gone` in a record and everything in it as the
/// node's former label, where the record is, or sits in, a retired or superseded decision.
fn former_labels(record: &mut Rec, gone: &BTreeMap<String, String>, in_settled: bool) {
    let in_settled = in_settled || (record.kind == Kind::Decision && settled(record));
    let (references, children) = node_refs(record);
    for child in children {
        former_labels(child, gone, in_settled);
    }
    if !in_settled {
        return;
    }
    for item in references {
        if let Some(label) = item.text().and_then(|key| gone.get(key)) {
            *item = former_label(label);
        }
    }
}

/// Writes each reference to a former label of `returned` in a record and everything in it as
/// the key of the node that came back.
fn returned_keys(record: &mut Rec, returned: &BTreeMap<String, String>) {
    let (references, children) = node_refs(record);
    for child in children {
        returned_keys(child, returned);
    }
    for item in references {
        let key = match &*item {
            Val::Record(former) => former
                .text(field::FORMER_LABEL)
                .and_then(|label| returned.get(label)),
            _ => None,
        };
        if let Some(key) = key {
            *item = Val::scalar(key);
        }
    }
}

/// `{formerLabel: <label>}` (AC-809).
fn former_label(label: &str) -> Val {
    let mut record = Rec::new(Kind::FormerLabel);
    record.set_text(field::FORMER_LABEL, label);
    Val::Record(record)
}

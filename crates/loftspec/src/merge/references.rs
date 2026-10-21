//! The references the tree merger's records hold that the reader resolves (F-32): a record one
//! side deleted and the other left as it was is kept, as the other side holds it, with a
//! conflict over `record`, while a record the merge keeps references it; and a field both sides
//! changed apart, whose base value names a record both sides deleted, holds ours' value beside
//! its conflict. Either may make the merge keep a record that references another, so both run to
//! a fixed point, after which every reference names a record the merge keeps, but one that no
//! place can take, which leaves the result unreadable. The merge driver,
//! which sees one file, keeps no record for a reference.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::Side;
use super::exist::{Fate, keep};
use super::flat::Parent;
use super::place::Placed;
use crate::codec::schema::{Item, Shape, Target};
use crate::codec::{ConflictRec, Rec, SideShape, Val, holder_path, side_shape, variant_holds};
use crate::identity::Key;

/// Keeps every record a side deleted that a record the merge keeps references, and gives ours'
/// value to every field whose base value names a record both sides deleted. The records both
/// deleted stay deleted, so each kept record is looked at once: those the merge keeps first, and
/// then, pass by pass, those a pass kept, until a pass keeps none.
pub(super) fn keep_referenced(
    top: Option<&mut Rec>,
    records: &mut BTreeMap<Key, Rec>,
    fates: &mut BTreeMap<Key, Fate>,
    places: &BTreeMap<Key, Placed>,
) {
    let gone: BTreeSet<Key> = fates
        .iter()
        .filter(|(_, fate)| matches!(fate, Fate::Gone))
        .map(|(key, _)| *key)
        .collect();
    let gone = |key: &Key| gone.contains(key);
    let tentative = |fates: &BTreeMap<Key, Fate>| {
        fates
            .values()
            .any(|fate| matches!(fate, Fate::Tentative(_)))
    };
    let mut referenced = Vec::new();
    if let Some(top) = top {
        take_ours(top, &gone);
        referenced.extend(references(top));
    }
    let mut work: Vec<Key> = kept(fates).collect();
    while !work.is_empty() {
        let scan = tentative(fates);
        for key in &work {
            let record = records.get_mut(key).expect("a fate is a record's");
            take_ours(record, &gone);
            if scan {
                referenced.extend(references(record));
            }
        }
        let deleted: Vec<(Key, Side)> = referenced
            .drain(..)
            .filter_map(|key| match fates.get(&key) {
                Some(Fate::Tentative(side)) if placed(key, fates, places) => Some((key, *side)),
                _ => None,
            })
            .collect();
        if deleted.is_empty() {
            return;
        }
        let before: BTreeSet<Key> = kept(fates).collect();
        keep(fates, places, deleted);
        work = kept(fates).filter(|key| !before.contains(key)).collect();
    }
}

/// The keys of the records `fates` keeps.
fn kept(fates: &BTreeMap<Key, Fate>) -> impl Iterator<Item = Key> + '_ {
    fates
        .iter()
        .filter(|(_, fate)| fate.kept())
        .map(|(key, _)| *key)
}

/// Whether the record `key` has a place, and so has each record above it that a side deleted:
/// a record that a side changed the kind of its parent may have none, and is not kept.
fn placed(key: Key, fates: &BTreeMap<Key, Fate>, places: &BTreeMap<Key, Placed>) -> bool {
    let mut at = key;
    loop {
        let Some(here) = places.get(&at) else {
            return false;
        };
        match here.place.0 {
            Parent::Key(parent) if matches!(fates.get(&parent), Some(Fate::Tentative(_))) => {
                at = parent;
            }
            _ => return true,
        }
    }
}

/// The keys that a record's own fields reference and that the reader resolves: the nodes and
/// decisions its edges and node references name. A former label names no record.
fn references(record: &Rec) -> Vec<Key> {
    let mut keys = Vec::new();
    fields_of(record, &mut keys);
    keys
}

fn fields_of(record: &Rec, keys: &mut Vec<Key>) {
    for (name, value) in &record.fields {
        if let Some((_, field)) = record.kind.field(name) {
            value_of(value, field.shape, keys);
        }
    }
}

fn value_of(value: &Val, shape: Shape, keys: &mut Vec<Key>) {
    match (value, shape.item()) {
        (Val::List(items), _) => {
            for item in items {
                value_of(item, Shape::One(shape.item()), keys);
            }
        }
        (Val::Record(record), Item::Record(_)) => fields_of(record, keys),
        (Val::Scalar { text, .. }, item) if resolved(item) => {
            keys.extend(Key::parse(text).ok());
        }
        _ => {}
    }
}

/// Whether the reader resolves a reference of this item to a node or a decision: an edge or a
/// node reference. A decision's option, driver or successor it leaves to the validator.
const fn resolved(item: Item) -> bool {
    matches!(
        item.target(),
        Some(Target::Node | Target::Component | Target::Product | Target::NodeOrDecision)
    )
}

/// Gives ours' value to each field of `record` whose conflict kept a base value naming a record
/// that `gone` says both sides deleted, and to every field the sides changed apart in the
/// unkeyed record that holds it where that record then forms none of its variants. A side's own
/// value names no record that side deleted, so ours' names none that both did, unless ours does
/// not read.
fn take_ours(record: &mut Rec, gone: &impl Fn(&Key) -> bool) {
    let dangling: Vec<ConflictRec> = record
        .conflicts
        .iter()
        .filter(|conflict| names_gone(record, conflict, gone))
        .cloned()
        .collect();
    for conflict in &dangling {
        record.set_at_path(&conflict.field, conflict.ours.clone());
        let holder = holder_path(&conflict.field);
        if record.holder_of(&conflict.field).is_some_and(variant_holds) {
            continue;
        }
        let siblings: Vec<ConflictRec> = record
            .conflicts
            .iter()
            .filter(|each| holder_path(&each.field) == holder)
            .cloned()
            .collect();
        for sibling in siblings {
            record.set_at_path(&sibling.field, sibling.ours);
        }
    }
}

/// Whether the value `record` holds at the field `conflict` names references a record `gone`
/// says both sides deleted.
fn names_gone(record: &Rec, conflict: &ConflictRec, gone: &impl Fn(&Key) -> bool) -> bool {
    let Some(SideShape::Field(shape)) = side_shape(record.kind, &conflict.field) else {
        return false;
    };
    let Some(value) = record.at_path(&conflict.field) else {
        return false;
    };
    let mut keys = Vec::new();
    value_of(value, shape, &mut keys);
    keys.iter().any(gone)
}

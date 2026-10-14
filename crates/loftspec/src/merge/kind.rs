//! A keyed record's own fields where its versions differ in kind (F-32). The kind merges as one
//! value: the side that changed it wins, base where the sides changed it apart, and ours where
//! base lacks the record. Each version is then read as that kind, keeping the fields that kind
//! defines as the version's own kind does and base's value of every other field, and the record
//! merges field by field as if it had kept its kind.

use std::collections::{BTreeMap, BTreeSet};

use super::flat::{Flat, Parent, keyed_list};
use super::record::merge_own;
use super::same::same_record;
use crate::codec::schema::Kind;
use crate::codec::{Rec, side_shape, variant_holds};
use crate::identity::Key;

/// Merges the own fields of the record keyed `key`, which ours and theirs both hold.
pub(super) fn merge_kinds(
    key: Key,
    sides: &[Flat; 3],
    sure: &BTreeSet<Key>,
    base: Option<&Rec>,
    ours: &Rec,
    theirs: &Rec,
) -> Rec {
    let kind = merged_kind(key, sides, sure, base, ours, theirs);
    let base = base.map(|base| as_kind(base, kind, None));
    let read = |version: &Rec| as_kind(version, kind, base.as_ref());
    let (ours, theirs) = (read(ours), read(theirs));
    let merged = merge_own(base.as_ref(), &ours, &theirs);
    if variant_holds(&merged) {
        return merged;
    }
    // Read as the merged kind, a version of another kind may form none of its variants; the
    // first version that forms one is then taken whole, as ours, theirs, then base.
    [&ours, &theirs]
        .into_iter()
        .chain(base.as_ref())
        .find(|version| variant_holds(version))
        .unwrap_or(&ours)
        .clone()
}

/// The record's kind in the result. A kind that lacks a list in which a side holds a record
/// beneath it that the merge surely keeps gives way to the next version's that has every such
/// list; a record the merge deletes needs no list.
fn merged_kind(
    key: Key,
    sides: &[Flat; 3],
    sure: &BTreeSet<Key>,
    base: Option<&Rec>,
    ours: &Rec,
    theirs: &Rec,
) -> Kind {
    let versions = match base {
        _ if ours.kind == theirs.kind => [
            ours.kind,
            theirs.kind,
            base.map_or(ours.kind, |base| base.kind),
        ],
        Some(base) if base.kind == ours.kind => [theirs.kind, ours.kind, base.kind],
        Some(base) if base.kind == theirs.kind => [ours.kind, theirs.kind, base.kind],
        Some(base) => [base.kind, ours.kind, theirs.kind],
        None => [ours.kind, theirs.kind, ours.kind],
    };
    let beneath: BTreeSet<&str> = children(key, sides)
        .filter(|(_, child)| sure.contains(child))
        .map(|(name, _)| name)
        .collect();
    let holds = |kind: &Kind| beneath.iter().all(|name| keyed_list(*kind, name).is_some());
    versions.into_iter().find(holds).unwrap_or(versions[0])
}

/// Each record a side holds directly beneath the record keyed `key`, with the list it is in.
fn children(key: Key, sides: &[Flat; 3]) -> impl Iterator<Item = (&'static str, Key)> + '_ {
    sides[1..]
        .iter()
        .flat_map(|side| &side.lists)
        .filter(move |((parent, _), _)| *parent == Parent::Key(key))
        .flat_map(|((_, name), keys)| keys.iter().map(|child| (*name, *child)))
}

/// The keys of the records the merge keeps whatever becomes of their places: those both sides
/// hold or base lacks, those the one side that holds them changed in their own fields or their
/// place, and those that hold such a record beneath them.
pub(super) fn surely_kept(sides: &[Flat; 3]) -> BTreeSet<Key> {
    let mut known = BTreeMap::new();
    let keys = sides[1..]
        .iter()
        .flat_map(|side| side.nodes.keys().copied());
    keys.filter(|key| kept(*key, sides, &mut known)).collect()
}

/// Whether the merge surely keeps the record keyed `key`; `known` holds each answer found so
/// far.
fn kept(key: Key, sides: &[Flat; 3], known: &mut BTreeMap<Key, bool>) -> bool {
    if let Some(kept) = known.get(&key) {
        return *kept;
    }
    // A record met again on its way down, through moves that nest records apart, adds nothing.
    known.insert(key, false);
    let [base, ours, theirs] = sides.each_ref().map(|side| side.nodes.get(&key));
    let kept = match (base, ours.or(theirs)) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(_), Some(_)) if ours.is_some() && theirs.is_some() => true,
        (Some(base), Some(kept)) => !same_record(&base.own, &kept.own) || base.place != kept.place,
    } || children(key, sides)
        .collect::<Vec<_>>()
        .into_iter()
        .any(|(_, child)| kept(child, sides, known));
    known.insert(key, kept);
    kept
}

/// `record` read as a record of `kind`: the fields, and the conflicts, whose values `kind`
/// shapes as the record's own kind does, and base's value of each field the record's kind does
/// not define so; a side that changed the kind has not deleted such a field.
fn as_kind(record: &Rec, kind: Kind, base: Option<&Rec>) -> Rec {
    let alike = |path: &str| {
        let shape = side_shape(kind, path);
        shape.is_some() && shape == side_shape(record.kind, path)
    };
    let mut read = Rec {
        kind,
        position: record.position,
        fields: record
            .fields
            .iter()
            .filter(|(name, _)| alike(name))
            .cloned()
            .collect(),
        conflicts: record
            .conflicts
            .iter()
            .filter(|conflict| alike(&conflict.field))
            .cloned()
            .collect(),
    };
    for (name, value) in base.map_or(&[][..], |base| &base.fields[..]) {
        if !alike(name) {
            read.set(name, value.clone());
        }
    }
    read
}

//! A keyed record's own fields where its versions differ in kind (F-32). The kind merges as one
//! value: the side that changed it wins, base where the sides changed it apart, and ours where
//! base lacks the record. Each version is then read as that kind, keeping the fields that kind
//! defines as the version's own kind does and base's value of every other field, and the record
//! merges field by field as if it had kept its kind.

use std::collections::BTreeSet;

use super::flat::{Flat, Parent, keyed_list};
use super::record::merge_own;
use crate::codec::schema::Kind;
use crate::codec::{Rec, side_shape, variant_holds};
use crate::identity::Key;

/// Merges the own fields of the record keyed `key`, which ours and theirs both hold.
pub(super) fn merge_kinds(
    key: Key,
    sides: &[Flat; 3],
    base: Option<&Rec>,
    ours: &Rec,
    theirs: &Rec,
) -> Rec {
    let kind = merged_kind(key, sides, base, ours, theirs);
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

/// The record's kind in the result. A kind that lacks a list in which a side holds records
/// beneath the record gives way to the next version's that has every such list.
fn merged_kind(key: Key, sides: &[Flat; 3], base: Option<&Rec>, ours: &Rec, theirs: &Rec) -> Kind {
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
    let beneath: BTreeSet<&str> = sides[1..]
        .iter()
        .flat_map(|side| &side.lists)
        .filter(|((parent, _), keys)| *parent == Parent::Key(key) && !keys.is_empty())
        .map(|((_, name), _)| *name)
        .collect();
    let holds = |kind: &Kind| beneath.iter().all(|name| keyed_list(*kind, name).is_some());
    versions.into_iter().find(holds).unwrap_or(versions[0])
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

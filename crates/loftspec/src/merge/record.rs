//! The merge of one record over its fields (D-11 F-32): a field one side changed takes the
//! change, a field both changed alike takes it once, and a field both changed apart keeps its
//! base value and gains a conflict on the record that holds it (RQ-120..RQ-122). An unkeyed
//! record inside it merges field by field under a dotted path.

use super::conflict::Host;
use super::list::merge_list;
use super::same::{same, same_record};
use crate::codec::schema::{Field, Item, Shape};
use crate::codec::{Rec, Val, variant_holds};

/// One field's value on each side, and whether base has the record at all.
struct Sides<'a> {
    base: Option<&'a Val>,
    ours: Option<&'a Val>,
    theirs: Option<&'a Val>,
    base_record: bool,
}

/// Merges a record that holds conflicts: its fields, then the conflicts already on it, merged
/// as a set, followed by those the merge writes.
pub(super) fn merge_host(base: Option<&Rec>, ours: &Rec, theirs: &Rec) -> Rec {
    let mut conflicts = carried(base, ours, theirs);
    let mut host = Host::root(&mut conflicts);
    let mut record = merge_fields(base, ours, theirs, &mut host);
    record.conflicts = conflicts;
    record
}

/// The conflicts the inputs already hold: one on both sides is written once, one that a side
/// removed is removed, and one that a side added is added.
fn carried(base: Option<&Rec>, ours: &Rec, theirs: &Rec) -> Vec<Rec> {
    let holds = |record: &Rec, conflict: &Rec| {
        record
            .conflicts
            .iter()
            .any(|each| same_record(each, conflict))
    };
    let kept = |conflict: &Rec| match base {
        Some(base) if holds(base, conflict) => holds(ours, conflict) && holds(theirs, conflict),
        _ => true,
    };
    let ours_kept = ours.conflicts.iter().filter(|conflict| kept(conflict));
    let theirs_added = theirs
        .conflicts
        .iter()
        .filter(|conflict| !holds(ours, conflict) && kept(conflict));
    ours_kept.chain(theirs_added).cloned().collect()
}

/// Merges the fields of one record; its conflicts go to `host`. A record whose merged fields
/// form none of its variants keeps its base and conflicts on each field either side changed.
pub(super) fn merge_fields(
    base: Option<&Rec>,
    ours: &Rec,
    theirs: &Rec,
    host: &mut Host<'_>,
) -> Rec {
    let kind = ours.kind;
    let mark = host.conflicts.len();
    let mut record = Rec::new(kind);
    for each in kind.fields() {
        let sides = sides(each, base, ours, theirs);
        if let Some(value) = merge_field(each, &sides, host) {
            record.set(each.name, value);
        }
    }
    if variant_holds(&record) {
        return record;
    }
    host.conflicts.truncate(mark);
    // The host merge sets the record's own conflicts; an unkeyed record holds none.
    let record = base.unwrap_or(ours).clone();
    for each in kind.fields() {
        let sides = sides(each, base, ours, theirs);
        if !same(sides.ours, sides.base) || !same(sides.theirs, sides.base) {
            host.conflict(each.name, [sides.base, sides.ours, sides.theirs]);
        }
    }
    record
}

fn sides<'a>(each: &Field, base: Option<&'a Rec>, ours: &'a Rec, theirs: &'a Rec) -> Sides<'a> {
    Sides {
        base: base.and_then(|record| record.get(each.name)),
        ours: ours.get(each.name),
        theirs: theirs.get(each.name),
        base_record: base.is_some(),
    }
}

fn merge_field(each: &Field, sides: &Sides<'_>, host: &mut Host<'_>) -> Option<Val> {
    if same(sides.ours, sides.theirs) {
        return sides.ours.cloned();
    }
    if same(sides.ours, sides.base) {
        return sides.theirs.cloned();
    }
    if same(sides.theirs, sides.base) {
        return sides.ours.cloned();
    }
    match (each.shape, sides.base, sides.ours, sides.theirs) {
        (Shape::List(_), base, ours, theirs) => Some(Val::List(merge_list(
            each,
            [list(base), list(ours), list(theirs)],
            host,
        ))),
        (Shape::One(Item::Record(_)), base, Some(Val::Record(ours)), Some(Val::Record(theirs))) => {
            // A sub-record base lacks merges as one both sides added (S7).
            let base = match base {
                Some(Val::Record(base)) => Some(base),
                _ => None,
            };
            Some(Val::Record(merge_fields(
                base,
                ours,
                theirs,
                &mut host.nest(each.name),
            )))
        }
        _ => {
            host.conflict(each.name, [sides.base, sides.ours, sides.theirs]);
            if sides.base_record {
                sides.base.cloned()
            } else {
                sides.ours.cloned()
            }
        }
    }
}

fn list(value: Option<&Val>) -> &[Val] {
    match value {
        Some(Val::List(items)) => items,
        _ => &[],
    }
}

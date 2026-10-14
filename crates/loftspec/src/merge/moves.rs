//! Records moved to another parent within one file (F-32). Before the merge of the file's
//! records, each moved record is matched with its base record by its key and merged three-way
//! on its own; the result is put at the parent the merge settles on in one side's version and
//! taken out of the others, so that the merge of the file writes it once. The parent is the
//! moving side's; the one both sides moved it to; or, where they moved it apart, its base
//! parent, with a conflict over `parent` on the record.

use std::collections::{BTreeMap, BTreeSet};

use super::Input;
use super::conflict::conflict;
use super::places::{Place, Places, put, take};
use super::record::merge_host;
use super::same::same_conflict;
use crate::codec::{ConflictRec, Rec, Val, field};
use crate::identity::Key;

/// Where the merge puts one record a side moved, and which side's version takes it.
#[derive(Debug, Clone)]
struct Plan {
    to: Place,
    host: Input,
    /// Each side's parent, where the sides put the record apart: base, ours, theirs.
    apart: Option<[Option<String>; 3]>,
}

/// Merges every record a side moved within the file and puts it at the parent the merge settles
/// on, in ours or theirs, taking it out of the other versions.
///
/// # Errors
///
/// The key of a moved record whose parent neither side holds any more, which the merge would
/// otherwise lose.
pub(super) fn normalise(
    mut base: Option<&mut Rec>,
    ours: &mut Rec,
    theirs: &mut Rec,
) -> Result<(), Key> {
    let sides = [
        base.as_deref().map(Places::of).unwrap_or_default(),
        Places::of(ours),
        Places::of(theirs),
    ];
    let mut plans = plans(&sides);
    settle_cycles(&mut plans, &sides);
    for key in order(&plans, &sides) {
        let plan = &plans[&key];
        let copies = [
            base.as_deref_mut().and_then(|tree| take(tree, &key)),
            take(ours, &key),
            take(theirs, &key),
        ];
        let Some(record) = merged(&key, copies, plan) else {
            continue;
        };
        let (host, other) = match plan.host {
            Input::Theirs => (&mut *theirs, &mut *ours),
            Input::Base | Input::Ours => (&mut *ours, &mut *theirs),
        };
        // A parent that a record moved earlier took into the other version is found there.
        if let Err(record) = put(host, record, &plan.to)
            && put(other, record, &plan.to).is_err()
        {
            return Err(Key::parse(&key).expect("a key the reader checked"));
        }
    }
    Ok(())
}

/// A plan for each key that a side carries at another parent than base, or that both sides
/// added at different parents.
fn plans(sides: &[Places; 3]) -> BTreeMap<String, Plan> {
    let [base, ours, theirs] = sides;
    let keys: BTreeSet<&String> = ours.placed().chain(theirs.placed()).collect();
    keys.into_iter()
        .filter_map(|key| {
            let places = [base.get(key), ours.get(key), theirs.get(key)];
            plan(places, sides).map(|plan| (key.clone(), plan))
        })
        .collect()
}

fn plan(places: [Option<&Place>; 3], sides: &[Places; 3]) -> Option<Plan> {
    let [base, ours, theirs] = places;
    let moved = |side: Option<&Place>| match (base, side) {
        (Some(base), Some(side)) => !side.same_parent(base),
        _ => false,
    };
    let to = |place: &Place, host: Input| Plan {
        to: place.clone(),
        host,
        apart: None,
    };
    match (base, ours, theirs) {
        (Some(_), Some(at), Some(other)) if moved(ours) && moved(theirs) => {
            if at.same_parent(other) {
                Some(to(at, Input::Ours))
            } else {
                Some(apart(places, sides))
            }
        }
        (None, Some(at), Some(other)) if !at.same_parent(other) => Some(apart(places, sides)),
        (_, Some(at), _) if moved(ours) => Some(to(at, Input::Ours)),
        (_, _, Some(at)) if moved(theirs) => Some(to(at, Input::Theirs)),
        _ => None,
    }
}

/// A record the sides put at different parents: it stays at its base parent in the version of
/// a side that still holds that parent, ours first, and otherwise goes where ours put it; a
/// conflict over `parent` names each side's parent.
fn apart(places: [Option<&Place>; 3], sides: &[Places; 3]) -> Plan {
    let [base, ours, theirs] = places;
    let held = |side: &Places, host: Input| {
        base.filter(|base| side.keys.contains(&base.parent))
            .map(|base| (base, host))
    };
    let (to, host) = held(&sides[1], Input::Ours)
        .or_else(|| held(&sides[2], Input::Theirs))
        .or_else(|| ours.map(|ours| (ours, Input::Ours)))
        .or_else(|| theirs.map(|theirs| (theirs, Input::Theirs)))
        .expect("a side holds a record it moved");
    Plan {
        to: to.clone(),
        host,
        apart: Some(places.map(|place| place.map(|place| place.parent.clone()))),
    }
}

/// The record merged from the versions that hold it. One that a side deleted is kept as the
/// moving side wrote it, with a conflict over `record` (AC-473); one the sides put apart gains a
/// conflict over `parent`.
fn merged(key: &str, [base, ours, theirs]: [Option<Rec>; 3], plan: &Plan) -> Option<Rec> {
    let mut record = match (ours, theirs) {
        (Some(ours), Some(theirs)) => merge_host(base.as_ref(), &ours, &theirs),
        (Some(kept), None) => deleted(key, base.is_some(), kept, Input::Theirs),
        (None, Some(kept)) => deleted(key, base.is_some(), kept, Input::Ours),
        (None, None) => return None,
    };
    if let Some(parents) = &plan.apart {
        let sides = parents
            .each_ref()
            .map(|parent| parent.as_deref().map(Val::scalar));
        hold(&mut record, ConflictRec::new(field::PARENT, sides));
    }
    Some(record)
}

/// A record `deleter` deleted and the other side moved.
fn deleted(key: &str, in_base: bool, mut kept: Rec, deleter: Input) -> Rec {
    if in_base {
        let key = Val::scalar(key);
        let null = Val::Null {
            position: kept.position,
        };
        let sides = match deleter {
            Input::Ours => [Some(&key), Some(&null), Some(&key)],
            Input::Base | Input::Theirs => [Some(&key), Some(&key), Some(&null)],
        };
        hold(&mut kept, conflict(field::RECORD, sides));
    }
    kept
}

/// Gives a record a conflict, unless it holds that conflict already.
fn hold(record: &mut Rec, conflict: ConflictRec) {
    if !record
        .conflicts
        .iter()
        .any(|held| same_conflict(held, &conflict))
    {
        record.conflicts.push(conflict);
    }
}

/// The planned keys, each after every planned key that sits inside it in any version, so that a
/// record is taken out of the versions only once the records moved within it have been.
fn order(plans: &BTreeMap<String, Plan>, sides: &[Places; 3]) -> Vec<String> {
    let mut pending: Vec<&String> = plans.keys().collect();
    let mut order = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let next = pending
            .iter()
            .position(|key| !holds_pending(key, &pending, sides))
            .unwrap_or(0);
        order.push(pending.remove(next).clone());
    }
    order
}

/// Whether another pending key sits inside `key` in any version.
fn holds_pending(key: &str, pending: &[&String], sides: &[Places; 3]) -> bool {
    pending
        .iter()
        .any(|other| *other != key && sides.iter().any(|places| places.within(other, key)))
}

/// Moves that would put a record inside itself, each side moving one record into the other,
/// stay at their base parents with a conflict over `parent`. A record base lacks has no parent
/// to stay at; its plan is dropped, and the merge refuses the key it then carries twice.
fn settle_cycles(plans: &mut BTreeMap<String, Plan>, sides: &[Places; 3]) {
    while let Some(cycle) = find_cycle(plans, sides) {
        let mut settled = false;
        for key in cycle {
            let places = sides.each_ref().map(|places| places.get(&key));
            let Some(plan) = plans.get(&key) else {
                continue;
            };
            match places[0] {
                Some(base) if !plan.to.same_parent(base) => {
                    let stay = apart(places, sides);
                    settled |= !stay.to.same_parent(&plan.to);
                    plans.insert(key, stay);
                }
                Some(_) => {}
                None => {
                    plans.remove(&key);
                    settled = true;
                }
            }
        }
        if !settled {
            return;
        }
    }
}

/// The keys of one cycle of parents that the plans would make, if any.
fn find_cycle(plans: &BTreeMap<String, Plan>, sides: &[Places; 3]) -> Option<Vec<String>> {
    let parent = |key: &str| -> Option<&str> {
        plans
            .get(key)
            .map(|plan| plan.to.parent.as_str())
            .or_else(|| {
                sides
                    .iter()
                    .find_map(|places| places.get(key))
                    .map(|place| place.parent.as_str())
            })
    };
    for start in plans.keys() {
        let mut path = vec![start.clone()];
        let mut at = parent(start);
        while let Some(key) = at {
            if key == start {
                return Some(path);
            }
            if path.iter().any(|seen| seen == key) {
                break;
            }
            path.push(key.to_owned());
            at = parent(key);
        }
    }
    None
}

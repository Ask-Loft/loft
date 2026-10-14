//! Which keyed records the merge keeps (F-32, RQ-119): each record both sides hold or one side
//! added; none that only base holds; and a record one side deleted only where the other side
//! changed it, or where the merge keeps a record beneath it (AC-660). The record is then kept
//! as the other side wrote it, its other records included, and carries a conflict over
//! `record`; a record kept only as part of it carries none.
//!
//! A record counts as changed by its own fields and conflicts, its place, and its position in
//! a list in authored order: never by the records beneath it, and never by the order a sorted
//! list keeps.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::Side;
use super::flat::{Flat, Node, Parent};
use super::place::Placed;
use super::tree::Lists;
use crate::identity::Key;

/// What becomes of one keyed record.
#[derive(Debug, Clone, Copy)]
pub(super) enum Fate {
    /// Kept: both sides hold it, or base lacks it.
    Kept,
    /// Kept although the side deleted it, with a conflict over `record`: the other side changed
    /// it, or the merge keeps a record beneath it.
    Despite(Side),
    /// Kept although a side deleted it, as part of a record kept despite that deletion.
    Beneath,
    /// Deleted by the side and left as it was by the other: it ends as gone unless the merge
    /// keeps it.
    Tentative(Side),
    /// Gone: deleted by both sides.
    Gone,
}

impl Fate {
    pub(super) const fn kept(self) -> bool {
        matches!(self, Self::Kept | Self::Despite(_) | Self::Beneath)
    }
}

/// The fate of every key any version holds, given each record's merged place and the lists
/// filtered to the records each holds after the merge. A record left `Tentative` is gone.
pub(super) fn decide(
    sides: &[Flat; 3],
    places: &BTreeMap<Key, Placed>,
    lists: &Lists,
) -> BTreeMap<Key, Fate> {
    let keys: BTreeSet<Key> = sides
        .iter()
        .flat_map(|side| side.nodes.keys().copied())
        .collect();
    let mut fates: BTreeMap<Key, Fate> = keys
        .into_iter()
        .map(|key| (key, seed(key, sides, lists)))
        .collect();
    let parent = |key: &Key| match places.get(key).map(|placed| placed.place.0) {
        Some(Parent::Key(parent)) => Some(parent),
        _ => None,
    };
    promote(&mut fates, &parent);
    keep_beneath(&mut fates, &parent);
    fates
}

/// Bottom up: each record a side deleted that holds a record the merge keeps is kept despite
/// that deletion.
fn promote(fates: &mut BTreeMap<Key, Fate>, parent: &dyn Fn(&Key) -> Option<Key>) {
    let kept: Vec<Key> = fates
        .iter()
        .filter(|(_, fate)| fate.kept())
        .map(|(key, _)| *key)
        .collect();
    for key in kept {
        let mut at = key;
        while let Some(above) = parent(&at) {
            let Some(Fate::Tentative(side)) = fates.get(&above).copied() else {
                break;
            };
            fates.insert(above, Fate::Despite(side));
            at = above;
        }
    }
}

/// Top down: each record a side deleted beneath one kept despite a deletion is kept, so that
/// the record is as the keeping side wrote it.
fn keep_beneath(fates: &mut BTreeMap<Key, Fate>, parent: &dyn Fn(&Key) -> Option<Key>) {
    let holds = |fate: Option<&Fate>| matches!(fate, Some(Fate::Despite(_) | Fate::Beneath));
    loop {
        let held: Vec<Key> = fates
            .iter()
            .filter(|(key, fate)| {
                matches!(fate, Fate::Tentative(_))
                    && parent(key).is_some_and(|above| holds(fates.get(&above)))
            })
            .map(|(key, _)| *key)
            .collect();
        if held.is_empty() {
            return;
        }
        fates.extend(held.into_iter().map(|key| (key, Fate::Beneath)));
    }
}

/// The fate of a record from the versions that hold it alone.
fn seed(key: Key, sides: &[Flat; 3], lists: &Lists) -> Fate {
    let [base, ours, theirs] = sides.each_ref().map(|side| side.nodes.get(&key));
    match (base, ours, theirs) {
        (Some(_), Some(_), Some(_)) | (None, _, _) => Fate::Kept,
        (Some(_), None, None) => Fate::Gone,
        (Some(base), Some(kept), None) => deleted(key, base, kept, Side::Theirs, lists),
        (Some(base), None, Some(kept)) => deleted(key, base, kept, Side::Ours, lists),
    }
}

/// A record `deleter` deleted: kept despite it where the other side changed the record.
fn deleted(key: Key, base: &Node, kept: &Node, deleter: Side, lists: &Lists) -> Fate {
    if kept.changed_from(base) || lists.moved(deleter.other(), &kept.place, key) {
        Fate::Despite(deleter)
    } else {
        Fate::Tentative(deleter)
    }
}

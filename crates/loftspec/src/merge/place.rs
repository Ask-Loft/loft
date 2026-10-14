//! Where each record goes (F-32): its place, the parent and the list that hold it, merged as
//! one value across the versions. One side's move wins, alike moves coalesce, and moves apart
//! leave the record at its base parent with a conflict over `parent`; a record both sides added
//! apart goes where ours put it, with a `parent` conflict without `base`. A place whose parent
//! neither side holds, or whose list cannot take the record, falls back to the base place and
//! then to ours' (or theirs'); so does each record of a cycle the merged places would make, to a
//! fixed point, with a `parent` conflict.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::parent_conflict;
use super::flat::{Flat, Parent, Place, accepts};
use super::same::same_value;
use crate::codec::ConflictRec;
use crate::codec::schema::Kind;
use crate::identity::Key;

/// A record's merged place, and the `parent` conflict it carries where the sides put it apart
/// or the merge put it elsewhere than it chose.
#[derive(Debug, Clone)]
pub(super) struct Placed {
    pub(super) place: Place,
    pub(super) conflict: Option<ConflictRec>,
}

/// The places a record may take, in the order the merge falls back through them, and which it
/// takes.
struct Ladder {
    chosen: Place,
    apart: bool,
    base: Option<Place>,
    levels: Vec<Place>,
    at: usize,
}

impl Ladder {
    fn place(&self) -> Place {
        self.levels[self.at]
    }

    fn can_step(&self) -> bool {
        self.at + 1 < self.levels.len()
    }

    /// Whether the record could step, and sits elsewhere than at its base place.
    fn should_step(&self) -> bool {
        self.can_step() && Some(self.place()) != self.base
    }
}

/// What a place must offer the record: a parent a side holds, whose merged kind's list takes
/// the record's merged kind. `kinds` holds the merged kind of each record a side holds.
struct Fits<'a> {
    kinds: &'a BTreeMap<Key, Kind>,
    top: Option<Kind>,
}

impl Fits<'_> {
    fn fits(&self, key: Key, (parent, name): Place) -> bool {
        let parent = match parent {
            Parent::Top => self.top,
            Parent::Key(parent) if parent == key => return false,
            Parent::Key(parent) => match self.kinds.get(&parent) {
                Some(kind) => Some(*kind),
                None => return false,
            },
        };
        accepts(parent, name, self.kinds[&key])
    }
}

/// The merged place of every record a side holds. `kinds` holds each such record's merged
/// kind, and `top` the kind of the file's top record (none for a registry).
///
/// # Errors
///
/// The key of a record no place can take: every place a version gave it is in a list its
/// merged parent's kind lacks.
pub(super) fn merge_places(
    sides: &[Flat; 3],
    kinds: &BTreeMap<Key, Kind>,
    top: Option<Kind>,
) -> Result<BTreeMap<Key, Placed>, Key> {
    let fits = Fits { kinds, top };
    let mut ladders = BTreeMap::new();
    for key in kinds.keys() {
        let ladder = ladder(*key, sides, &fits).ok_or(*key)?;
        ladders.insert(*key, ladder);
    }
    settle_cycles(&mut ladders)?;
    Ok(ladders
        .into_iter()
        .map(|(key, ladder)| {
            let place = ladder.place();
            let parents = sides.each_ref().map(|side| {
                let (parent, _) = side.place(&key)?;
                side.parent_value(parent)
            });
            // Sides that differ only in the list, as a change of kind moves a record, hold one
            // parent: a conflict over it would name no difference.
            let mut held = parents.iter().flatten();
            let first = held.next();
            let differ = held.any(|parent| first.is_none_or(|first| !same_value(first, parent)));
            let conflict = ((ladder.apart || place != ladder.chosen) && differ)
                .then(|| parent_conflict(parents));
            (key, Placed { place, conflict })
        })
        .collect())
}

/// The places a record may take: the one the three-way merge chooses, its base place, then
/// ours', or theirs' where ours lacks the record or ours' list cannot take its merged kind;
/// each only where it fits.
fn ladder(key: Key, sides: &[Flat; 3], fits: &Fits<'_>) -> Option<Ladder> {
    let [base, ours, theirs] = sides.each_ref().map(|side| side.place(&key));
    let (chosen, apart) = chosen(base, ours, theirs)?;
    let side = [ours, theirs]
        .into_iter()
        .flatten()
        .find(|place| fits.fits(key, *place));
    let mut levels: Vec<Place> = [Some(chosen), base, side]
        .into_iter()
        .flatten()
        .filter(|place| fits.fits(key, *place))
        .collect();
    levels.dedup();
    (!levels.is_empty()).then_some(Ladder {
        chosen,
        apart,
        base,
        levels,
        at: 0,
    })
}

/// The place the three-way merge chooses, and whether the sides put the record apart.
fn chosen(
    base: Option<Place>,
    ours: Option<Place>,
    theirs: Option<Place>,
) -> Option<(Place, bool)> {
    Some(match (ours, theirs) {
        (Some(ours), Some(theirs)) if ours == theirs => (ours, false),
        (Some(ours), Some(theirs)) => match base {
            Some(base) if ours == base => (theirs, false),
            Some(base) if theirs == base => (ours, false),
            Some(base) => (base, true),
            None => (ours, true),
        },
        (Some(only), None) | (None, Some(only)) => (only, false),
        (None, None) => return None,
    })
}

/// Steps each record on a cycle of merged parents to its next place until none is left: those
/// away from their base place first, so that a record at its base place stays there while
/// another can break the cycle. Each record steps at most twice, and at its last place follows
/// ours' tree, or theirs' where ours lacks it, neither of which has a cycle; only a record whose
/// kind changed can be left on one.
fn settle_cycles(ladders: &mut BTreeMap<Key, Ladder>) -> Result<(), Key> {
    loop {
        let parents = ladders
            .iter()
            .map(|(key, ladder)| (*key, ladder.place().0))
            .collect();
        let cycles = on_cycles(&parents);
        let Some(first) = cycles.first().copied() else {
            return Ok(());
        };
        let away = |key: &Key| ladders[key].should_step();
        let stepping: Vec<Key> = if cycles.iter().any(away) {
            cycles.into_iter().filter(away).collect()
        } else {
            cycles
                .into_iter()
                .filter(|key| ladders[key].can_step())
                .collect()
        };
        if stepping.is_empty() {
            return Err(first);
        }
        for key in stepping {
            ladders.get_mut(&key).expect("a record with a place").at += 1;
        }
    }
}

/// Every key on a cycle of parents.
fn on_cycles(parents: &BTreeMap<Key, Parent>) -> BTreeSet<Key> {
    let mut done = BTreeSet::new();
    let mut cycles = BTreeSet::new();
    for start in parents.keys() {
        let mut path: Vec<Key> = Vec::new();
        let mut next = Some(*start);
        while let Some(key) = next.filter(|key| !done.contains(key)) {
            if let Some(at) = path.iter().position(|each| *each == key) {
                cycles.extend(path[at..].iter().copied());
                break;
            }
            path.push(key);
            next = match parents.get(&key) {
                Some(Parent::Key(parent)) => Some(*parent),
                _ => None,
            };
        }
        done.extend(path);
    }
    cycles
}

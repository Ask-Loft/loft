//! The merge by key of one file (F-32, F-33): every keyed record merged on its own, matched by
//! its key wherever each version holds it. Its own fields merge field by field, its place as one
//! value; which records survive follows from what each side did to each record; then each list
//! is merged in order over the records it holds after the merge, and the tree is built back.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::{Side, hold, position_conflict, record_conflict};
use super::exist::{Fate, decide};
use super::flat::{Flat, Parent, Place, order_of, rebuild};
use super::kind::{merge_kinds, surely_kept};
use super::order::{merge_order, moved};
use super::place::{Placed, merge_places};
use super::record::merge_own;
use crate::codec::schema::{Kind, Order};
use crate::codec::{Content, Rec};
use crate::identity::Key;

/// Merges a record file or a registry by key.
///
/// # Errors
///
/// The key of a record no place can take (see [`merge_places`]), or that the merge keeps
/// without a place or a parent.
pub(super) fn merge(
    base: Option<&Content>,
    ours: &Content,
    theirs: &Content,
) -> Result<Content, Key> {
    let sides = [
        base.map_or_else(Flat::default, Flat::of),
        Flat::of(ours),
        Flat::of(theirs),
    ];
    let merged = merge_flat(&sides)?;
    Ok(rebuild(merged.top, merged.records, &merged.lists))
}

/// The merge of three versions by key: the top record's own fields, every record the merge
/// keeps with its own fields and its conflicts, and every list that holds one, in merged order.
pub(super) struct ByKey {
    pub(super) top: Option<Rec>,
    pub(super) records: BTreeMap<Key, Rec>,
    pub(super) lists: BTreeMap<Place, Vec<Key>>,
}

/// Merges three versions, each by key.
///
/// # Errors
///
/// As [`merge`].
pub(super) fn merge_flat(sides: &[Flat; 3]) -> Result<ByKey, Key> {
    let [base_top, ours_top, theirs_top] = sides.each_ref().map(|side| side.top.as_ref());
    let top = ours_top
        .zip(theirs_top)
        .map(|(ours, theirs)| merge_own(base_top, ours, theirs));
    let sure = surely_kept(sides);
    let mut records = merge_records(sides, &sure);
    let kinds = records
        .iter()
        .map(|(key, record)| (*key, record.kind))
        .collect();
    let top_kind = top.as_ref().map(|top| top.kind);
    let places = merge_places(sides, &kinds, top_kind, &sure)?;
    let lists = Lists::filtered(sides, &places, top_kind);
    let fates = decide(sides, &places, &lists);
    records.retain(|key, _| fates[key].kept());
    if let Some(key) = unplaced(&records, &places) {
        return Err(key);
    }
    for (key, record) in &mut records {
        if let Fate::Despite(deleter) = fates[key] {
            let conflict = record_conflict(record, deleter);
            hold(record, conflict);
        }
        if let Some(conflict) = &places[key].conflict {
            hold(record, conflict.clone());
        }
    }
    let lists = lists.merge(&places, &mut records);
    Ok(ByKey {
        top,
        records,
        lists,
    })
}

/// A record the merge keeps without a place, or whose parent it does not keep: one the merge
/// left without a place, sure that it would delete it.
fn unplaced(records: &BTreeMap<Key, Rec>, places: &BTreeMap<Key, Placed>) -> Option<Key> {
    records.keys().copied().find(|key| match places.get(key) {
        Some(at) => match at.place.0 {
            Parent::Key(parent) => !records.contains_key(&parent),
            Parent::Top => false,
        },
        None => true,
    })
}

/// The own fields of every record a side holds, merged.
fn merge_records(sides: &[Flat; 3], sure: &BTreeSet<Key>) -> BTreeMap<Key, Rec> {
    let [_, ours, theirs] = sides;
    let keys: BTreeSet<&Key> = ours.nodes.keys().chain(theirs.nodes.keys()).collect();
    keys.into_iter()
        .map(|key| {
            let [ours, theirs] =
                [ours, theirs].map(|side| side.nodes.get(key).map(|node| &node.own));
            let record = match (ours, theirs) {
                (Some(_), Some(_)) => merge_kinds(*key, sides, sure),
                (Some(only), None) | (None, Some(only)) => only.clone(),
                (None, None) => unreachable!("a key a side holds"),
            };
            (*key, record)
        })
        .collect()
}

/// Each version's lists, each filtered to the records the merge places in it and those no side
/// holds, so that a record moved elsewhere makes no neighbour look moved; the order each list
/// keeps; and the records ours and theirs each moved within each list in authored order.
pub(super) struct Lists {
    sides: [BTreeMap<Place, Vec<Key>>; 3],
    orders: BTreeMap<Place, Order>,
    moved: [BTreeMap<Place, BTreeSet<Key>>; 2],
}

impl Lists {
    fn filtered(sides: &[Flat; 3], places: &BTreeMap<Key, Placed>, top: Option<Kind>) -> Self {
        let mut orders = BTreeMap::new();
        let filtered = sides.each_ref().map(|side| {
            let mut lists = BTreeMap::new();
            for (place, keys) in &side.lists {
                let parent = match place.0 {
                    Parent::Top => top,
                    Parent::Key(parent) => side.nodes.get(&parent).map(|node| node.own.kind),
                };
                orders
                    .entry(*place)
                    .or_insert_with(|| order_of(parent, place.1));
                let here =
                    |key: &&Key| places.get(*key).is_none_or(|placed| placed.place == *place);
                lists.insert(*place, keys.iter().filter(here).copied().collect());
            }
            lists
        });
        let mut lists = Self {
            sides: filtered,
            orders,
            moved: Default::default(),
        };
        lists.moved = [1, 2].map(|side| lists.moved_in(side));
        lists
    }

    /// The records `side` holds elsewhere than base does in each list in authored order.
    fn moved_in(&self, side: usize) -> BTreeMap<Place, BTreeSet<Key>> {
        self.orders
            .keys()
            .filter(|place| self.authored(place))
            .map(|place| (*place, moved(self.list(0, place), self.list(side, place))))
            .collect()
    }

    fn list(&self, side: usize, place: &Place) -> &[Key] {
        self.sides[side].get(place).map_or(&[], Vec::as_slice)
    }

    fn authored(&self, place: &Place) -> bool {
        self.orders.get(place) == Some(&Order::Authored)
    }

    /// Whether `side` holds `key` elsewhere in the list `place` than base does, in a list in
    /// authored order.
    pub(super) fn moved(&self, side: Side, place: &Place, key: Key) -> bool {
        let side = match side {
            Side::Ours => 0,
            Side::Theirs => 1,
        };
        self.moved[side]
            .get(place)
            .is_some_and(|moved| moved.contains(&key))
    }

    /// Every list that holds a record after the merge, in merged order; a record both sides
    /// moved apart within its list gains a conflict over `position` (RQ-124).
    fn merge(
        &self,
        places: &BTreeMap<Key, Placed>,
        records: &mut BTreeMap<Key, Rec>,
    ) -> BTreeMap<Place, Vec<Key>> {
        let targets: BTreeSet<Place> = records.keys().map(|key| places[key].place).collect();
        targets
            .into_iter()
            .map(|place| (place, self.merge_list(place, records)))
            .collect()
    }

    fn merge_list(&self, place: Place, records: &mut BTreeMap<Key, Rec>) -> Vec<Key> {
        let [base, ours, theirs] = [0, 1, 2].map(|side| self.list(side, &place));
        // Each record the merge keeps has a place (`unplaced`), and each list holds only the
        // records placed in it (`Lists::filtered`): a record here survives if it is kept.
        let kept: &BTreeMap<Key, Rec> = records;
        let survives = |key: &Key| kept.contains_key(key);
        if !self.authored(&place) {
            // The writer sorts the list; which records it holds is all the merge settles.
            let mut seen = BTreeSet::new();
            return ours
                .iter()
                .chain(theirs)
                .chain(base)
                .filter(|key| survives(key) && seen.insert(**key))
                .copied()
                .collect();
        }
        let merged = merge_order(base, ours, theirs, &survives);
        for position in &merged.conflicts {
            if let Some(record) = records.get_mut(&position.element) {
                hold(record, position_conflict(position));
            }
        }
        merged.order
    }
}

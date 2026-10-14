//! The merge by key of one file (F-32, F-33): every keyed record merged on its own, matched by
//! its key wherever each version holds it. Its own fields merge field by field, its place as one
//! value; which records survive follows from what each side did to each record; then each list
//! is merged in order over the records it holds after the merge, and the tree is built back.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::{Side, hold, position_conflict, record_conflict};
use super::exist::{Fate, decide};
use super::flat::{Flat, Parent, Place, order_of, rebuild};
use super::kind::merge_kinds;
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
/// The key of a record no place can take (see [`merge_places`]).
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
    let [base_top, ours_top, theirs_top] = sides.each_ref().map(|side| side.top.as_ref());
    let top = ours_top
        .zip(theirs_top)
        .map(|(ours, theirs)| merge_own(base_top, ours, theirs));
    let mut records = merge_records(&sides);
    let kinds = records
        .iter()
        .map(|(key, record)| (*key, record.kind))
        .collect();
    let top_kind = top.as_ref().map(|top| top.kind);
    let places = merge_places(&sides, &kinds, top_kind)?;
    let lists = Lists::filtered(&sides, &places, top_kind);
    let fates = decide(&sides, &places, &lists);
    records.retain(|key, _| fates[key].kept());
    for (key, record) in &mut records {
        if let Fate::Despite(deleter) = fates[key] {
            let conflict = record_conflict(record, deleter);
            hold(record, conflict);
        }
        if let Some(conflict) = &places[key].conflict {
            hold(record, conflict.clone());
        }
    }
    let merged = lists.merge(&places, &mut records);
    Ok(rebuild(top, &records, &merged))
}

/// The own fields of every record a side holds, merged.
fn merge_records(sides: &[Flat; 3]) -> BTreeMap<Key, Rec> {
    let [base, ours, theirs] = sides;
    let keys: BTreeSet<&Key> = ours.nodes.keys().chain(theirs.nodes.keys()).collect();
    keys.into_iter()
        .map(|key| {
            let [base, ours, theirs] =
                [base, ours, theirs].map(|side| side.nodes.get(key).map(|node| &node.own));
            let record = match (ours, theirs) {
                (Some(ours), Some(theirs)) => merge_kinds(*key, sides, base, ours, theirs),
                (Some(only), None) | (None, Some(only)) => only.clone(),
                (None, None) => unreachable!("a key a side holds"),
            };
            (*key, record)
        })
        .collect()
}

/// Each version's lists, each filtered to the records the merge places in it and those no side
/// holds, so that a record moved elsewhere makes no neighbour look moved; and the order each
/// list keeps.
pub(super) struct Lists {
    sides: [BTreeMap<Place, Vec<Key>>; 3],
    orders: BTreeMap<Place, Order>,
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
        Self {
            sides: filtered,
            orders,
        }
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
            Side::Ours => 1,
            Side::Theirs => 2,
        };
        self.authored(place) && moved(self.list(0, place), self.list(side, place)).contains(&key)
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
            .map(|place| (place, self.merge_list(place, places, records)))
            .collect()
    }

    fn merge_list(
        &self,
        place: Place,
        places: &BTreeMap<Key, Placed>,
        records: &mut BTreeMap<Key, Rec>,
    ) -> Vec<Key> {
        let [base, ours, theirs] = [0, 1, 2].map(|side| self.list(side, &place));
        let kept: &BTreeMap<Key, Rec> = records;
        let survives = |key: &Key| {
            kept.contains_key(key) && places.get(key).is_some_and(|at| at.place == place)
        };
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
                hold(record, position_conflict(position, Key::to_string));
            }
        }
        merged.order
    }
}

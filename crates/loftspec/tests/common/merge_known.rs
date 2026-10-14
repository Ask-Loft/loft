//! The known differences between `merge_file` and `merge_file_by_key`, which is to replace it:
//! the classes of case on which the merge by key gives another result on purpose. The
//! differential tests compare the two merges on every other case.

use loftspec::codec::schema::{Kind, field};

use loftspec::merge::order::moved;

use super::edit_tree::{ENTRIES, Top, Y};
use super::merge_defects::{
    a_key_changes_kind, a_side_renames_an_entry_and_the_other_deletes_one,
    moves_a_record_into_a_list_it_also_reorders, moves_a_record_into_one_it_held,
    moves_on_the_two_sides_form_a_cycle,
};
use super::merge_keys::{Flat, Versions, deletions, unchanged};

/// Whether three versions of a file fall in a class of case.
type Class = fn(&Versions) -> bool;

/// Each known difference, by name, with the class of case it covers.
const KNOWN: [(&str, Class); 10] = [
    // `merge_file` brings back a record one side deleted when the other deleted or moved out
    // only records beneath it; the merge by key deletes it.
    (
        "a record one side deletes and the other only empties",
        a_side_empties_a_record_the_other_side_deleted,
    ),
    // `merge_file` sees a registry's re-sorting as a move and keeps a deleted neighbour of a
    // renamed entry with a conflict; the merge by key counts no move in a sorted list.
    (
        "a renamed registry entry beside a deleted one",
        a_side_renames_an_entry_and_the_other_deletes_one,
    ),
    // `merge_file` puts a record the sides moved apart back into one side's version after its
    // base predecessor, ahead of the other side's insertions there; the merge by key leaves it
    // at its base position, after them.
    (
        "a record moved apart, back at its base list",
        a_record_both_sides_move_apart,
    ),
    // `merge_file` takes a record both sides moved into one list out of theirs, so a record
    // theirs placed right after it there follows its predecessor instead; the merge by key keeps
    // it after the record.
    (
        "a record both sides move into one list",
        a_side_places_a_record_after_one_both_sides_move_alike,
    ),
    // `merge_file` drops an assessment one side deleted and the other changed; the merge by key
    // keeps its base value with a conflict over the list.
    (
        "an assessment deleted and changed",
        an_assessment_one_side_deletes_and_the_other_changes,
    ),
    // `merge_file` reaches a field the other kind lacks; the merge by key merges the record's
    // own fields as one value.
    ("a key whose kind changes", a_key_changes_kind),
    // `merge_file` may refuse the key it then carries twice, and is not symmetric; the merge by
    // key steps each record of the cycle to its base parent, or else to ours'.
    (
        "moves on the two sides forming a cycle",
        moves_on_the_two_sides_form_a_cycle,
    ),
    // `merge_file` merges such a side with itself into something else; the merge by key gives
    // the side.
    ("a side moving a record into one it held", |versions| {
        either_side(versions, moves_a_record_into_one_it_held)
    }),
    (
        "a side moving a record into a list it also reorders",
        |versions| either_side(versions, moves_a_record_into_a_list_it_also_reorders),
    ),
    // `merge_file` writes one `record` conflict on the outermost record a side deleted that the
    // other side changed anywhere beneath; the merge by key writes one on each record that side
    // deleted which the other changed or which holds a record the merge keeps.
    (
        "a record kept beneath two a side deleted",
        a_side_keeps_a_record_beneath_two_the_other_side_deleted,
    ),
];

/// The name of the first known difference the versions fall in; none where both merges must
/// agree.
pub fn known_difference(versions: &Versions) -> Option<&'static str> {
    KNOWN
        .iter()
        .find(|(_, applies)| applies(versions))
        .map(|(name, _)| *name)
}

fn either_side(versions: &Versions, class: fn(&Flat, &Flat) -> bool) -> bool {
    versions
        .sides()
        .into_iter()
        .any(|side| class(&versions.base, side))
}

/// Whether one side deletes a record of base that the other side left as it was but for
/// records beneath it that it deleted, or that either side moved out: every record that stays
/// beneath it, where the deleting side does not hold it elsewhere, is as base has it.
fn a_side_empties_a_record_the_other_side_deleted(versions: &Versions) -> bool {
    let Versions {
        base, ours, theirs, ..
    } = versions;
    deletions(base, ours, theirs).into_iter().any(|(key, _)| {
        let (deleting, keeping) = if ours.has(&key) {
            (theirs, ours)
        } else {
            (ours, theirs)
        };
        let stays = staying(&key, deleting, keeping);
        untouched(versions, keeping, &key)
            && stays
                .iter()
                .all(|inner| untouched(versions, keeping, inner))
            && base
                .beneath(&key)
                .iter()
                .any(|inner| !stays.contains(inner))
    })
}

/// The records beneath `key` on the keeping side that stay there in the merge: those the
/// deleting side does not hold at another place, and the records beneath them.
fn staying(key: &str, deleting: &Flat, keeping: &Flat) -> Vec<String> {
    let leaves = |inner: &String| {
        deleting.keyed.get(inner).is_some_and(|there| {
            Some(&there.place) != keeping.keyed.get(inner).map(|kept| &kept.place)
        })
    };
    let mut stays = Vec::new();
    let mut open = vec![key.to_owned()];
    while let Some(parent) = open.pop() {
        let children = keeping
            .keyed
            .iter()
            .filter(|(_, kept)| kept.place.0.as_deref() == Some(parent.as_str()))
            .map(|(child, _)| child.clone())
            .filter(|child| !leaves(child));
        for child in children {
            stays.push(child.clone());
            open.push(child);
        }
    }
    stays
}

/// Whether a side holds a record of base as base does, as both merges see it: its own fields
/// and its place the same, and, in a list in authored order, no move of it among the records
/// that no version holds elsewhere.
fn untouched(versions: &Versions, side: &Flat, key: &str) -> bool {
    let base = &versions.base;
    let (Some(before), Some(after)) = (base.keyed.get(key), side.keyed.get(key)) else {
        return false;
    };
    if before.own != after.own || before.place != after.place {
        return false;
    }
    let place = &before.place;
    let here = |each: &&String| {
        [&versions.base, &versions.ours, &versions.theirs]
            .into_iter()
            .all(|flat| {
                flat.keyed
                    .get(*each)
                    .is_none_or(|keyed| keyed.place == *place)
            })
    };
    let list = |flat: &Flat| -> Vec<String> {
        let list = flat.lists.get(place).cloned().unwrap_or_default();
        list.iter().filter(here).cloned().collect()
    };
    place.1 == ENTRIES || !moved(&list(base), &list(side)).contains(key)
}

/// Whether both sides move one record of base to different lists than base's and each other's,
/// and a side changes its base list otherwise than by taking it out.
fn a_record_both_sides_move_apart(versions: &Versions) -> bool {
    let Versions {
        base, ours, theirs, ..
    } = versions;
    base.keyed.iter().any(|(key, before)| {
        let (Some(one), Some(other)) = (ours.keyed.get(key), theirs.keyed.get(key)) else {
            return false;
        };
        let apart =
            one.place != before.place && other.place != before.place && one.place != other.place;
        let list = |flat: &Flat| flat.lists.get(&before.place).cloned().unwrap_or_default();
        let mut held = list(base);
        held.retain(|each| each != key);
        let rearranged = |side: &Flat| list(side) != held;
        apart && (rearranged(ours) || rearranged(theirs))
    })
}

/// Whether both sides move one record of base into the same other list, and a side holds right
/// after it there a record that base's list lacks or that the side moved.
fn a_side_places_a_record_after_one_both_sides_move_alike(versions: &Versions) -> bool {
    let Versions {
        base, ours, theirs, ..
    } = versions;
    base.keyed.iter().any(|(key, before)| {
        let (Some(one), Some(other)) = (ours.keyed.get(key), theirs.keyed.get(key)) else {
            return false;
        };
        let place = &one.place;
        let held = base.lists.get(place).cloned().unwrap_or_default();
        let placed_after = |side: &Flat| {
            let list = side.lists.get(place).cloned().unwrap_or_default();
            let at = list.iter().position(|each| each == key);
            let next = at.and_then(|at| list.get(at + 1));
            next.is_some_and(|next| !held.contains(next) || moved(&held, &list).contains(next))
        };
        one.place == other.place
            && *place != before.place
            && (placed_after(ours) || placed_after(theirs))
    })
}

/// Whether one side deletes a record of base and the record holding it, where the other side
/// changed the record, or keeps a record beneath it that the merge keeps: one both sides hold,
/// one base lacks, or one the other side changed.
fn a_side_keeps_a_record_beneath_two_the_other_side_deleted(versions: &Versions) -> bool {
    let Versions {
        base, ours, theirs, ..
    } = versions;
    [(ours, theirs), (theirs, ours)]
        .into_iter()
        .any(|(deleting, keeping)| {
            base.keyed.keys().any(|key| {
                let Some(kept) = keeping.keyed.get(key) else {
                    return false;
                };
                let inner = !deleting.has(key)
                    && kept
                        .place
                        .0
                        .as_ref()
                        .is_some_and(|parent| base.has(parent) && !deleting.has(parent));
                let held = |each: &String| {
                    deleting.has(each) || !base.has(each) || !unchanged(base, keeping, each)
                };
                inner && (held(key) || keeping.beneath(key).iter().any(held))
            })
        })
}

/// Whether, in a decision file, one side deletes an assessment of base that the other changes.
fn an_assessment_one_side_deletes_and_the_other_changes(versions: &Versions) -> bool {
    if versions.top != Top::Record(Kind::Decision) {
        return false;
    }
    let [base, ours, theirs] = [&versions.base, &versions.ours, &versions.theirs].map(assessments);
    base.iter().any(|(pair, before)| {
        let changed = |side: &Vec<(Pair, Y)>| {
            side.iter()
                .any(|(other, after)| other == pair && after != before)
        };
        let lacks = |side: &Vec<(Pair, Y)>| side.iter().all(|(other, _)| other != pair);
        (lacks(&ours) && changed(&theirs)) || (lacks(&theirs) && changed(&ours))
    })
}

/// An assessment's option and driver.
type Pair = (String, String);

/// The assessments of a decision version, each by its option and driver.
fn assessments(flat: &Flat) -> Vec<(Pair, Y)> {
    let Some(decision) = flat.keyed.values().find(|keyed| keyed.place.1.is_empty()) else {
        return Vec::new();
    };
    let text = |assessment: &Y, name: &str| {
        assessment
            .get(name)
            .and_then(Y::text)
            .unwrap_or_default()
            .to_owned()
    };
    decision
        .own
        .get(field::ASSESSMENTS)
        .map(Y::items)
        .unwrap_or_default()
        .iter()
        .map(|each| {
            let pair = (text(each, field::OPTION), text(each, field::DRIVER));
            (pair, each.clone())
        })
        .collect()
}

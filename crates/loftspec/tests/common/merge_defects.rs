//! The classes of case today's file merge gets wrong, which the merge properties leave out.
//! Each is lifted when the merge is rewritten as one merge by key.

use loftspec::codec::schema::field;

use super::edit_tree::Top;
use super::merge_cases::Case;
use super::merge_keys::{Flat, Kept, deletions, flatten};

fn versions(case: &Case) -> [Flat; 3] {
    [&case.base, &case.ours, &case.theirs].map(|bytes| flatten(case.fixture.top, bytes))
}

/// Today's engine: a key whose record is of one kind in one version and of another kind in
/// another (one side deleted it and added a record of another kind under the same key, or
/// both sides added it as different kinds) reaches a field the other kind lacks, and a debug
/// build panics on the field access; lifted when the merge is rewritten.
pub fn a_key_changes_kind(case: &Case) -> bool {
    let [base, ours, theirs] = versions(case);
    let kinds = |key: &String| {
        [&base, &ours, &theirs]
            .into_iter()
            .filter_map(|flat| flat.keyed.get(key).map(|keyed| keyed.kind))
            .collect::<Vec<_>>()
    };
    base.keyed
        .keys()
        .chain(ours.keyed.keys())
        .chain(theirs.keyed.keys())
        .any(|key| kinds(key).windows(2).any(|pair| pair[0] != pair[1]))
}

/// Today's engine: when the two sides' moves together would put a record beneath itself (each
/// side moving one record into the other), the merge may refuse the key it then carries twice
/// with `DuplicateKey`. A record base lacks has no base parent to stay at; and for two records
/// base holds, the outcome depends on which side moves which: the same pair of moves merges
/// with two `parent` conflicts one way round and is refused the other; lifted when the merge is
/// rewritten.
pub fn moves_on_the_two_sides_form_a_cycle(case: &Case) -> bool {
    let [_, ours, theirs] = versions(case);
    let parents = |key: &str| -> Vec<String> {
        [&ours, &theirs]
            .into_iter()
            .filter_map(|flat| flat.keyed.get(key)?.place.0.clone())
            .collect()
    };
    let keys: Vec<&String> = ours.keyed.keys().chain(theirs.keyed.keys()).collect();
    keys.into_iter().any(|start| {
        let mut seen = vec![start.clone()];
        let mut open = parents(start);
        while let Some(key) = open.pop() {
            if &key == start {
                return true;
            }
            if !seen.contains(&key) {
                open.extend(parents(&key));
                seen.push(key);
            }
        }
        false
    })
}

/// Today's engine: a record one side deleted, which the other side left as it was but for
/// records beneath it that it deleted, is brought back with a `record` conflict, since whether
/// the keeping side changed it compares the whole subtree; lifted when the merge is rewritten.
pub fn a_side_deletes_beneath_a_record_the_other_side_deleted(case: &Case) -> bool {
    let [base, ours, theirs] = versions(case);
    deletions(&base, &ours, &theirs)
        .into_iter()
        .any(|(_, kept)| kept == Kept::DeletedBeneath)
}

/// Today's engine: in a registry, which is sorted by name, renaming an entry re-sorts it, so a
/// neighbouring entry looks moved; when the other side deleted that neighbour, the deletion is
/// kept back with a `record` conflict; lifted when the merge is rewritten.
pub fn a_side_renames_an_entry_and_the_other_deletes_one(case: &Case) -> bool {
    let [base, ours, theirs] = versions(case);
    let renames = |side: &Flat| {
        side.keyed.iter().any(|(key, entry)| {
            base.keyed
                .get(key)
                .is_some_and(|before| before.own.get(field::NAME) != entry.own.get(field::NAME))
        })
    };
    let deletes = |side: &Flat| base.keyed.keys().any(|key| !side.has(key));
    case.fixture.top == Top::Registry
        && ((renames(&ours) && deletes(&theirs)) || (renames(&theirs) && deletes(&ours)))
}

/// Today's engine: when both sides alike move a record out of its parent to another list and
/// then move that parent, still holding another record, into the record it held, the merge
/// writes a `record` conflict on the parent whose theirs side is null, though neither side
/// deleted it; lifted when the merge is rewritten.
pub fn moves_a_record_into_one_it_held(case: &Case, side: &[u8]) -> bool {
    let base = flatten(case.fixture.top, &case.base);
    let side = flatten(case.fixture.top, side);
    side.keyed.keys().any(|key| {
        base.beneath(key)
            .iter()
            .any(|inner| side.beneath(inner).contains(key))
    })
}

/// Today's engine: when both sides alike move a record into a list from another list and
/// reorder the records that list held in base, the merge puts the moved record into ours only,
/// so theirs seems to have moved a neighbour elsewhere, and it writes a `position` conflict and
/// an order neither side has; lifted when the merge is rewritten.
pub fn moves_a_record_into_a_list_it_also_reorders(case: &Case, side: &[u8]) -> bool {
    let base = flatten(case.fixture.top, &case.base);
    let side = flatten(case.fixture.top, side);
    side.lists.iter().any(|(place, keys)| {
        let moved_in = keys.iter().any(|key| {
            base.keyed
                .get(key)
                .is_some_and(|before| &before.place != place)
        });
        let held = base.lists.get(place).cloned().unwrap_or_default();
        let kept: Vec<&String> = held.iter().filter(|key| keys.contains(key)).collect();
        let now: Vec<&String> = keys.iter().filter(|key| held.contains(key)).collect();
        moved_in && kept != now
    })
}

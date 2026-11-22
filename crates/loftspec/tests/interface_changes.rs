//! What a move of a copy's pin did to the promise it copies: the nodes of the Interface the
//! newer state added, changed and removed against the older, each node by its own content.

use crate::common::copies::{copy_at, owner};
use crate::common::interfaces::INTERFACE;
use crate::common::{criterion, key};
use loftspec::diff::interface_changes;
use loftspec::ids::{CriterionId, NodeId};
use loftspec::publish::publish_labels;

fn criterion_id(n: u32) -> NodeId {
    NodeId::Criterion(CriterionId::new(key(n)))
}

#[test]
fn a_reworded_criterion_is_changed_and_its_requirement_and_interface_are_not() {
    let older = copy_at(&owner(), "format", 3);
    let mut reworded = owner();
    reworded.interfaces[0].requirements[0].criteria[0].name =
        "The answer names its owner.".to_owned();
    let newer = copy_at(&reworded, "format", 4);
    let changes = interface_changes(&older.interface, &newer.interface);
    assert_eq!(changes.changed, [criterion_id(INTERFACE + 2)]);
    assert!(
        changes.added.is_empty() && changes.removed.is_empty(),
        "{changes:?}"
    );
}

#[test]
fn a_criterion_the_newer_state_adds_or_drops_is_added_or_removed() {
    let older = copy_at(&owner(), "format", 3);
    let mut moved = owner();
    let criteria = &mut moved.interfaces[0].requirements[0].criteria;
    criteria.clear();
    criteria.push(criterion(
        key(INTERFACE + 3),
        "The answer is the latest state.",
    ));
    publish_labels(&mut moved);
    let newer = copy_at(&moved, "format", 4);
    let changes = interface_changes(&older.interface, &newer.interface);
    assert_eq!(changes.added, [criterion_id(INTERFACE + 3)]);
    assert_eq!(changes.removed, [criterion_id(INTERFACE + 2)]);
    assert!(changes.changed.is_empty(), "{changes:?}");
    assert!(interface_changes(&older.interface, &older.interface).is_empty());
}

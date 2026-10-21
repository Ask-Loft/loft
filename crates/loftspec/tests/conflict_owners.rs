//! The owner of the conflicts a record holds (Settling a conflict), and the own record view it
//! names, so that a caller matches conflicts to records without deriving the owner itself.

use std::collections::BTreeSet;

use loftspec::codec::{self, Rec, RecordKind, conflict_owner, own_records, read_tree};
use loftspec::conflict::ConflictOwner;
use loftspec::ids::{ConsequenceId, DecisionId, ElementId, RequirementId, TagId};

use crate::common::generated::rich_document;
use crate::common::{key, small_document};

/// Every record of the small document's written tree, at any depth.
fn records() -> Vec<Rec> {
    let tree = read_tree(&codec::write(&small_document())).expect("the tree reads");
    let mut all = Vec::new();
    let mut pending: Vec<Rec> = tree
        .files
        .values()
        .flat_map(|(_, content)| content.records().cloned())
        .collect();
    while let Some(record) = pending.pop() {
        pending.extend(record.children().cloned());
        all.push(record);
    }
    all
}

fn owners() -> BTreeSet<ConflictOwner> {
    records().iter().filter_map(conflict_owner).collect()
}

#[test]
fn the_owner_of_a_keyed_records_conflicts_is_what_its_kind_and_key_name() {
    let owners = owners();
    let node = ElementId::Node(RequirementId::new(key(4)).into());
    for owner in [
        ConflictOwner::Element(node),
        ConflictOwner::Element(ElementId::Decision(DecisionId::new(key(20)))),
        ConflictOwner::Consequence(ConsequenceId::new(key(25))),
        ConflictOwner::Tag(TagId::new(key(30))),
    ] {
        assert!(owners.contains(&owner), "{owner:?} in {owners:?}");
    }
}

#[test]
fn the_owner_of_the_manifests_and_the_bindings_conflicts_is_that_record() {
    let owners = owners();
    assert!(owners.contains(&ConflictOwner::Manifest));
    assert!(owners.contains(&ConflictOwner::Labels));
}

#[test]
fn a_record_that_holds_no_conflicts_has_no_owner() {
    let unowned: Vec<_> = records()
        .into_iter()
        .filter(|record| conflict_owner(record).is_none())
        .map(|record| record.kind)
        .collect();
    assert_ne!(unowned, []);
    for kind in unowned {
        assert!(!kind.hosts_conflicts(), "{kind:?}");
    }
}

#[test]
fn the_owner_of_a_nodes_a_decisions_or_an_entrys_conflicts_names_its_own_record_view() {
    let document = rich_document();
    let tree = read_tree(&codec::write(&document)).expect("the tree reads");
    let views = own_records(&document);
    let mut named = BTreeSet::new();
    let mut pending: Vec<&Rec> = tree
        .files
        .values()
        .flat_map(|(_, content)| content.records())
        .collect();
    while let Some(record) = pending.pop() {
        pending.extend(record.children());
        if let Some(view) = conflict_owner(record).and_then(ConflictOwner::own_record) {
            assert!(views.contains_key(&view), "{view:?}");
            named.insert(view);
        }
    }
    // A binding's conflicts are the bindings record's, whose owner names no view.
    let all: BTreeSet<(RecordKind, _)> = views
        .keys()
        .copied()
        .filter(|(kind, _)| *kind != RecordKind::Binding)
        .collect();
    assert_eq!(named, all);
}

#[test]
fn the_owner_of_a_decision_parts_the_manifests_and_the_bindings_conflicts_names_no_view() {
    for owner in [
        ConflictOwner::Consequence(ConsequenceId::new(key(25))),
        ConflictOwner::Manifest,
        ConflictOwner::Labels,
    ] {
        assert_eq!(owner.own_record(), None, "{owner:?}");
    }
}

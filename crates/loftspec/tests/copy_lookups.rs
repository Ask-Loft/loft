//! What a copy answers about itself and the document that holds it: its Interface's key, the
//! names of its nodes, the node a label names under a handle, the references into it, and how it
//! differs from the copy its pinned state gives.

use std::collections::BTreeSet;

use crate::common::copies::{COPY, HANDLE, copy_at, owner, requiring, state_root};
use crate::common::interfaces::INTERFACE;
use crate::common::key;
use crate::common::text::replaced;
use loftspec::codec;
use loftspec::copy::{CopyDifference, labeled, references};
use loftspec::document::{Edge, EdgeKind};
use loftspec::identity::Label;
use loftspec::ids::{ComponentId, ElementId, InterfaceId};

#[test]
fn a_copy_names_its_interface_and_its_nodes_by_handle_and_label() {
    let copy = copy_at(&owner(), HANDLE, 3);
    assert_eq!(copy.key(), key(INTERFACE));
    let label = copy.labels[&key(INTERFACE)];
    assert_eq!(copy.name_of(key(INTERFACE)), format!("{HANDLE}:{label}"));
    assert_eq!(copy.name_of(key(0x999)), format!("{HANDLE}:{}", key(0x999)));
}

#[test]
fn a_label_under_a_handle_names_the_node_its_copies_bind_to_it_and_no_other_handles() {
    let document = requiring();
    let copy = &document.copies[0];
    let (criterion, label) = copy.labels.iter().last().unwrap();
    assert_eq!(labeled(&document.copies, HANDLE, *label), Some(*criterion));
    assert_eq!(labeled(&document.copies, "elsewhere", *label), None);
    let unbound = Label::parse("AC-999").unwrap();
    assert_eq!(labeled(&document.copies, HANDLE, unbound), None);
}

#[test]
fn the_references_into_a_copy_are_the_edges_that_name_its_nodes() {
    let mut document = requiring();
    let provides = Edge::Provides {
        component: ComponentId::new(key(6)),
        interface: InterfaceId::new(key(INTERFACE)),
    };
    document.edges.push(provides);
    let keys: BTreeSet<_> = document.copies[0].keys().into_iter().collect();
    let found = references(&document.edges, &keys);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].source, ElementId::from(ComponentId::new(key(6))));
    assert_eq!(found[0].kind.field(), "provides");
    assert!(EdgeKind::Provides.crosses() && !EdgeKind::Realizes.crosses());
    assert_eq!(EdgeKind::Subject.field(), "subjects");
}

#[test]
fn a_copy_differs_from_its_pinned_copy_first_by_state_root_then_interface_then_label() {
    let served = copy_at(&owner(), HANDLE, 3);
    assert_eq!(served.difference(&served), None);
    let other_root = copy_at(&owner(), HANDLE, 4);
    assert_eq!(
        other_root.difference(&served),
        Some(CopyDifference::StateRoot {
            held: state_root(4),
            served: state_root(3),
        })
    );
    let mut unlabeled = served.clone();
    let (key, label) = unlabeled.labels.pop_first().unwrap();
    assert_eq!(
        unlabeled.difference(&served),
        Some(CopyDifference::Label {
            key,
            held: None,
            served: Some(label),
        })
    );
}

#[test]
fn a_copy_whose_interface_carries_no_key_is_refused() {
    let files = codec::write(&requiring());
    let interface = format!("  key: {}\n", key(INTERFACE));
    let keyless = replaced(&files, COPY, &interface, "");
    assert!(codec::read(&keyless).is_err());
}

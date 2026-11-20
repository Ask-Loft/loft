//! Interfaces take `IF` labels at publish, after the root Components and before the decisions,
//! and `spec/labels.yaml` writes the `IF` counter only once an Interface has been labeled.

use std::num::NonZeroU64;

use crate::common::interfaces::{INTERFACE, interface, with_interface};
use crate::common::publishing::allocated;
use crate::common::text::{path, text};
use crate::common::{component, key, label, root, slug, small_document};
use loftspec::codec;
use loftspec::identity::LabelPrefix;
use loftspec::ids::{DecisionId, InterfaceId};
use loftspec::publish::allocate;

#[test]
fn interfaces_are_labeled_after_the_root_components_and_before_the_decisions() {
    let mut document = with_interface();
    let mut later = root(component(key(INTERFACE + 0x20), "Zeta"));
    later.slug = slug("zeta");
    document.components.push(later);
    let mut decision = document.decisions[0].clone();
    decision.id = DecisionId::new(key(INTERFACE + 0x21));
    decision.slug = slug("another-decision");
    document.decisions.push(decision);
    assert_eq!(
        allocated(&mut document),
        ["AC-1", "CMP-2", "IF-1", "RQ-3", "AC-2", "ADR-2"]
    );
    assert_eq!(
        document.bindings.label_of(InterfaceId::new(key(INTERFACE))),
        Some(label("IF-1"))
    );
}

#[test]
fn interfaces_are_labeled_in_the_order_of_their_file_names() {
    let mut document = small_document();
    let mut later = interface(key(INTERFACE + 0x22), "Zeta feed", Vec::new());
    later.slug = slug("zeta-feed");
    let mut earlier = interface(key(INTERFACE + 0x23), "Alpha feed", Vec::new());
    earlier.slug = slug("alpha-feed");
    document.interfaces.extend([later, earlier]);
    assert_eq!(allocated(&mut document), ["AC-1", "IF-1", "IF-2"]);
    assert_eq!(
        document
            .bindings
            .label_of(InterfaceId::new(key(INTERFACE + 0x23))),
        Some(label("IF-1"))
    );
}

#[test]
fn the_interface_counter_is_written_after_the_decision_counter_once_an_interface_is_labeled() {
    let mut document = with_interface();
    allocate(&mut document);
    let labels = text(&codec::write(&document), "labels.yaml");
    assert!(labels.contains("  ADR: 2\n  IF: 2\n"), "{labels}");
}

#[test]
fn a_labels_file_without_an_interface_counter_reads_it_as_one() {
    let mut document = small_document();
    allocate(&mut document);
    let files = codec::write(&document);
    let labels = text(&files, "labels.yaml");
    assert!(!labels.contains("IF:"), "{labels}");
    let read = codec::read(&files).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(read.counters.next(LabelPrefix::Interface), NonZeroU64::MIN);
}

#[test]
fn a_publish_without_an_interface_writes_the_labels_file_it_wrote_before_interfaces() {
    let mut document = small_document();
    allocate(&mut document);
    let files = codec::write(&document);
    let read = codec::read(&files).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(
        codec::write(&read)[&path("labels.yaml")],
        files[&path("labels.yaml")]
    );
}

#[test]
fn interfaces_whose_slugs_differ_by_a_hyphened_suffix_are_labeled_as_their_file_names_sort() {
    let mut document = small_document();
    let mut shorter = interface(key(INTERFACE + 0x24), "A", Vec::new());
    shorter.slug = slug("a");
    let mut longer = interface(key(INTERFACE + 0x25), "A b", Vec::new());
    longer.slug = slug("a-b");
    document.interfaces.extend([shorter, longer]);
    // `a-b.yaml` sorts before `a.yaml`, since a hyphen sorts before a dot.
    assert_eq!(allocated(&mut document), ["AC-1", "IF-1", "IF-2"]);
    assert_eq!(
        document
            .bindings
            .label_of(InterfaceId::new(key(INTERFACE + 0x25))),
        Some(label("IF-1"))
    );
}

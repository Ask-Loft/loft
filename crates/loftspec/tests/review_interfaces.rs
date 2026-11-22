//! The Interfaces each Component of the review packet provides and requires (The review packet),
//! over the small document of `review`, whose Feature F-1 (3) realizes the Component CMP-1 (6).

use crate::common::copies::{SHELF, requiring_two};
use crate::common::interfaces::{
    INTERFACE, LATER, MODEL, interface, provided_and_required, provides, requires,
};
use crate::common::key;
use crate::review::{criterion_mut, ns, packed, unit};
use loftspec::document::{Document, Edge};
use loftspec::identity::Key;
use loftspec::review::{Change, Fact};

fn provided(fact: &Fact) -> Vec<Key> {
    fact.provides.iter().map(|id| id.key()).collect()
}

fn required(fact: &Fact) -> Vec<Key> {
    fact.requires.iter().map(|id| id.key()).collect()
}

/// `document` with criterion 5 renamed, so that F-1's unit is reviewed against a base.
fn renamed(mut document: Document) -> Document {
    "Every node record carries a key.".clone_into(&mut criterion_mut(&mut document).name);
    document
}

#[test]
fn each_component_carries_the_interfaces_it_provides_and_requires_in_key_order() {
    let mut document = requiring_two();
    document
        .interfaces
        .push(interface(key(LATER), "Zeta feed", Vec::new()));
    document.edges.extend([
        requires(MODEL, SHELF),
        provides(MODEL, LATER),
        requires(MODEL, INTERFACE),
    ]);
    let packet = packed(&document, None, None);
    let feature = unit(&packet, 3);
    let component = &feature.realizes[0].component.node;
    assert_eq!(provided(component), ns(&[LATER]));
    assert_eq!(required(component), ns(&[INTERFACE, SHELF]));
    assert_eq!(provided(&feature.scope.node), ns(&[]));
    assert_eq!(required(&feature.scope.node), ns(&[]));
}

#[test]
fn a_realized_component_whose_provides_alone_differ_is_changed() {
    let base = provided_and_required();
    let mut document = renamed(base.clone());
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Provides { .. }));
    let packet = packed(&document, Some(&base), None);
    let feature = unit(&packet, 3);
    assert_eq!(feature.scope.node.change, None);
    let component = &feature.realizes[0].component.node;
    assert_eq!(component.change, Some(Change::Changed));
    assert_eq!(provided(component), ns(&[]));
    assert_eq!(required(component), ns(&[LATER]));
}

#[test]
fn a_realized_component_whose_requires_alone_differ_is_changed() {
    let base = provided_and_required();
    let mut document = renamed(base.clone());
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Requires { .. }));
    let packet = packed(&document, Some(&base), None);
    let component = &unit(&packet, 3).realizes[0].component.node;
    assert_eq!(component.change, Some(Change::Changed));
    assert_eq!(required(component), ns(&[]));
}

#[test]
fn a_realized_component_whose_interfaces_are_the_base_s_is_unchanged() {
    let base = provided_and_required();
    let packet = packed(&renamed(base.clone()), Some(&base), None);
    let component = &unit(&packet, 3).realizes[0].component.node;
    assert_eq!(component.change, None);
    assert_eq!(provided(component), ns(&[INTERFACE]));
}

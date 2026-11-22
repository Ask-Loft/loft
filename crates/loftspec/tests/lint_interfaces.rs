//! The rules of Interfaces: `V2-012`, an Interface that holds no Requirement and so promises
//! nothing, and `V2-013`, an Interface that no Component of its document provides or requires.

use crate::common::interfaces::{INTERFACE, MODEL, interface, provides, requires, with_interface};
use crate::common::key;
use crate::common::lint::{findings, keys_of, severity};
use loftspec::document::{Document, Edge};
use loftspec::identity::Key;
use loftspec::lint::{RuleId, Severity};

/// No finding's keys.
const NONE: [Vec<Key>; 0] = [];

/// The small document whose root Component links to its Interface with `link`.
fn linked(link: fn(u32, u32) -> Edge) -> Document {
    let mut document = with_interface();
    document.edges.push(link(MODEL, INTERFACE));
    document
}

#[test]
fn an_interface_without_a_requirement_is_reported_once_by_its_key() {
    let mut document = linked(provides);
    document.interfaces[0].requirements.clear();
    assert_eq!(
        keys_of(&findings(&document), RuleId::V2012),
        [vec![key(INTERFACE)]]
    );
}

#[test]
fn an_interface_that_holds_a_requirement_promises_something() {
    assert_eq!(keys_of(&findings(&linked(provides)), RuleId::V2012), NONE);
}

#[test]
fn an_interface_that_promises_nothing_is_a_warning_by_default() {
    assert_eq!(severity(RuleId::V2012), Severity::Warning);
}

#[test]
fn an_interface_no_component_provides_or_requires_is_reported_once_by_its_key() {
    let mut document = linked(provides);
    document
        .interfaces
        .push(interface(key(INTERFACE + 0x30), "Audit feed", Vec::new()));
    assert_eq!(
        keys_of(&findings(&document), RuleId::V2013),
        [vec![key(INTERFACE + 0x30)]]
    );
    assert_eq!(
        keys_of(&findings(&with_interface()), RuleId::V2013),
        [vec![key(INTERFACE)]]
    );
}

#[test]
fn an_interface_one_component_provides_or_requires_is_not_reported_as_unlinked() {
    assert_eq!(keys_of(&findings(&linked(provides)), RuleId::V2013), NONE);
    assert_eq!(keys_of(&findings(&linked(requires)), RuleId::V2013), NONE);
}

#[test]
fn an_interface_no_component_links_to_is_info_by_default() {
    assert_eq!(severity(RuleId::V2013), Severity::Info);
}

#[test]
fn a_registry_term_an_interface_does_not_list_is_reported_on_the_interface() {
    let mut document = linked(provides);
    document.interfaces[0].name = "Identity lookup".to_owned();
    let found = findings(&document);
    let untagged = keys_of(&found, RuleId::Lq009);
    assert!(untagged.contains(&vec![key(INTERFACE)]), "{untagged:?}");
}

#[test]
fn a_copy_of_another_documents_interface_yields_no_v2_013_finding() {
    let document = crate::common::copies::requiring();
    assert!(document.interfaces.is_empty() && !document.copies.is_empty());
    assert_eq!(keys_of(&findings(&document), RuleId::V2013), NONE);
}

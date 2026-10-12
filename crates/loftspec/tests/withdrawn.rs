//! Withdrawn labels (D-11 F-2): a label a key lost to a publish that met another is recorded on
//! the key's binding, resolves to nothing, is never allocated again and enters no root.

mod common;

use common::text::{files, text};
use common::{key, label, small_document};
use loftspec::bindings::{BindingError, Bindings, Counters};
use loftspec::codec;
use loftspec::document::Document;
use loftspec::hash::hash_document;
use loftspec::identity::LabelPrefix;
use loftspec::ids::RequirementId;
use loftspec::validate::{ValidationError, validate};

fn requirement() -> RequirementId {
    RequirementId::new(key(4))
}

/// The small document with `RQ-7` withdrawn from its Requirement, which holds `RQ-2` and the alias
/// `RQ-1`.
fn withdrawn() -> Document {
    let mut document = small_document();
    document
        .bindings
        .add_withdrawn(requirement(), label("RQ-7"))
        .unwrap();
    document
}

#[test]
fn a_withdrawn_label_is_written_after_the_aliases_and_read_back() {
    let files = codec::write(&withdrawn());
    let labels = text(&files, "labels.yaml");
    assert!(
        labels.contains("    aliases:\n      - RQ-1\n    withdrawn:\n      - RQ-7\n"),
        "{labels}"
    );
    let read = codec::read(&files).unwrap();
    assert_eq!(read.bindings.withdrawn_of(requirement()), [label("RQ-7")]);
    assert_eq!(codec::write(&read), files);
}

#[test]
fn a_binding_without_a_withdrawn_label_omits_withdrawn() {
    assert!(!text(&files(), "labels.yaml").contains("withdrawn"));
}

#[test]
fn a_withdrawn_label_enters_no_root() {
    let plain = hash_document(&small_document()).unwrap();
    let with = hash_document(&withdrawn()).unwrap();
    assert_eq!(with.roots, plain.roots);
}

#[test]
fn a_withdrawn_label_resolves_to_no_element() {
    let document = withdrawn();
    assert_eq!(document.bindings.resolve(label("RQ-7")), None);
    assert_eq!(
        document.bindings.resolve(label("RQ-2")),
        Some(requirement().into())
    );
}

#[test]
fn a_withdrawn_label_is_never_allocated_again() {
    let counters = Counters::above(&withdrawn().bindings);
    assert_eq!(counters.next(LabelPrefix::Requirement).get(), 8);
}

#[test]
fn a_withdrawn_label_that_is_the_key_s_own_label_is_an_error_naming_the_key_and_the_label() {
    let mut document = small_document();
    let bindings = &mut document.bindings;
    bindings
        .add_withdrawn(requirement(), label("RQ-1"))
        .unwrap();
    let errors = validate(&document);
    let expected = BindingError::WithdrawnIsOwnLabel {
        element: requirement().into(),
        label: label("RQ-1"),
    };
    assert_eq!(errors, [ValidationError::Binding(expected.clone())]);
    let message = expected.to_string();
    assert!(
        message.contains("RQ-1") && message.contains(&key(4).to_string()),
        "{message}"
    );
}

#[test]
fn a_withdrawn_label_that_is_one_of_the_key_s_aliases_is_an_error() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(), label("RQ-1")).unwrap();
    bindings.add_alias(requirement(), label("RQ-2")).unwrap();
    bindings
        .add_withdrawn(requirement(), label("RQ-2"))
        .unwrap();
    assert_eq!(
        bindings.validate(),
        [BindingError::WithdrawnIsOwnLabel {
            element: requirement().into(),
            label: label("RQ-2"),
        }]
    );
}

#[test]
fn a_label_withdrawn_from_one_key_and_live_on_another_is_no_error() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(), label("RQ-1")).unwrap();
    bindings
        .add_withdrawn(requirement(), label("RQ-2"))
        .unwrap();
    bindings
        .bind(RequirementId::new(key(40)), label("RQ-2"))
        .unwrap();
    assert_eq!(bindings.validate(), Vec::new());
    assert_eq!(
        bindings.resolve(label("RQ-2")),
        Some(RequirementId::new(key(40)).into())
    );
}

#[test]
fn unbinding_a_key_returns_its_binding_and_frees_its_labels() {
    let mut bindings = Bindings::default();
    bindings.bind(requirement(), label("RQ-1")).unwrap();
    bindings.add_alias(requirement(), label("RQ-2")).unwrap();
    let binding = bindings.unbind(requirement()).unwrap();
    assert_eq!(
        (binding.label, binding.aliases),
        (label("RQ-1"), vec![label("RQ-2")])
    );
    assert_eq!(bindings.label_of(requirement()), None);
    assert_eq!(bindings.resolve(label("RQ-1")), None);
    assert_eq!(bindings.resolve(label("RQ-2")), None);
    assert_eq!(bindings.unbind(requirement()), None);
}

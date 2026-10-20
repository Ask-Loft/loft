//! Departed bindings (Labels and aliases): the binding of a key that left the document stays in
//! the bindings file marked departed, keeps every label it held, resolves to nothing and enters
//! no root, as the state its last publish recorded does not.

use crate::common::full::full_document;
use crate::common::{key, label, proposed_decision, small_document};
use loftspec::canonical::Digest;
use loftspec::codec;
use loftspec::decision::StateKind;
use loftspec::hash::hash_document;
use loftspec::ids::{DecisionId, RequirementId};
use loftspec::publish::allocate;

#[test]
fn a_departed_binding_enters_no_root() {
    let plain = small_document();
    let mut departed = plain.clone();
    let gone = DecisionId::new(key(90));
    departed.bindings.bind(gone, label("ADR-5")).unwrap();
    departed.bindings.depart(gone).unwrap();
    let roots = |document| hash_document(document).unwrap().roots;
    assert_eq!(roots(&departed), roots(&plain));
}

#[test]
fn a_bindings_published_state_and_history_enter_no_root() {
    let plain = small_document();
    let mut recorded = plain.clone();
    let history = "ab".repeat(32).parse::<Digest>().unwrap();
    let decision = DecisionId::new(key(20));
    recorded
        .bindings
        .record_published(decision, Some(StateKind::Accepted), Some(history))
        .unwrap();
    let roots = |document| hash_document(document).unwrap().roots;
    assert_eq!(roots(&recorded), roots(&plain));
}

#[test]
fn a_departed_label_resolves_to_no_element() {
    let document = full_document();
    assert_eq!(document.bindings.resolve(label("ADR-5")), None);
    assert_eq!(document.bindings.resolve(label("ADR-4")), None);
    assert_eq!(
        document.bindings.resolve(label("RQ-2")),
        Some(RequirementId::new(key(4)).into())
    );
}

#[test]
fn a_departed_binding_reads_back_with_its_labels_and_its_record() {
    let document = codec::read(&codec::write(&full_document())).unwrap();
    let binding = document
        .bindings
        .iter()
        .find(|binding| binding.element.key() == key(90))
        .unwrap();
    assert!(binding.departed);
    assert_eq!(binding.element, DecisionId::new(key(90)).into());
    assert_eq!(binding.label, label("ADR-5"));
    assert_eq!(binding.aliases, [label("ADR-4")]);
    assert_eq!(binding.withdrawn, [label("ADR-3")]);
    assert_eq!(binding.published_state, Some(StateKind::Accepted));
    assert_eq!(
        binding.published_history,
        Some("cd".repeat(32).parse::<Digest>().unwrap())
    );
}

#[test]
fn a_departed_label_is_never_allocated_to_another_key() {
    let mut document = small_document();
    let gone = DecisionId::new(key(90));
    document.bindings.bind(gone, label("ADR-5")).unwrap();
    document.bindings.depart(gone).unwrap();
    document
        .decisions
        .push(proposed_decision(key(91), "a-new-proposal"));
    let allocated = allocate(&mut document);
    let given = allocated
        .iter()
        .find(|given| given.element == DecisionId::new(key(91)).into());
    assert_eq!(given.map(|given| given.label), Some(label("ADR-6")));
    assert_eq!(document.bindings.label_of(gone), None);
}

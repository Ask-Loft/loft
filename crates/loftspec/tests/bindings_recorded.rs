//! Recording a binding, whole, as `spec/labels.yaml` holds it: its label whatever its prefix, its
//! aliases and withdrawn labels, its departure and what a publish recorded in it, so that a
//! caller who keeps bindings apart from the file builds them back binding by binding.

use loftspec::bindings::{Binding, BindingError, Bindings};
use loftspec::canonical::Digest;
use loftspec::codec;
use loftspec::decision::StateKind;
use loftspec::document::Document;
use loftspec::ids::{DecisionId, ElementId, FeatureId, RequirementId, SectionId};

use crate::common::{key, label, small_document};

fn binding(element: impl Into<ElementId>, text: &str) -> Binding {
    Binding {
        element: element.into(),
        label: label(text),
        departed: false,
        aliases: Vec::new(),
        withdrawn: Vec::new(),
        published_state: None,
        published_history: None,
    }
}

fn recorded(binding: Binding) -> Bindings {
    let mut bindings = Bindings::default();
    bindings.record(binding).expect("the binding is recorded");
    bindings
}

/// The one binding of `bindings`.
fn only(bindings: &Bindings) -> &Binding {
    let mut all = bindings.iter();
    let first = all.next().expect("a binding");
    assert!(all.next().is_none());
    first
}

#[test]
fn a_recorded_binding_keeps_a_label_whose_prefix_is_not_its_kinds() {
    let promoted: FeatureId = RequirementId::new(key(4)).promote();
    let bindings = recorded(binding(promoted, "RQ-2"));
    assert_eq!(only(&bindings).label, label("RQ-2"));
    assert_eq!(bindings.label_of(promoted), Some(label("RQ-2")));
    assert_eq!(bindings.resolve(label("RQ-2")), Some(promoted.into()));
}

#[test]
fn a_recorded_binding_keeps_its_aliases_and_withdrawn_labels_in_order() {
    let mut whole = binding(RequirementId::new(key(4)), "RQ-9");
    whole.aliases = vec![label("RQ-3"), label("RQ-1")];
    whole.withdrawn = vec![label("RQ-7"), label("RQ-5")];
    let bindings = recorded(whole.clone());
    assert_eq!(only(&bindings), &whole);
    assert_eq!(
        bindings.resolve(label("RQ-1")),
        Some(RequirementId::new(key(4)).into())
    );
    assert!(bindings.is_withdrawn(label("RQ-5")));
}

#[test]
fn a_recorded_binding_keeps_whether_it_is_departed() {
    let mut departed = binding(RequirementId::new(key(4)), "RQ-9");
    departed.departed = true;
    departed.aliases = vec![label("RQ-1")];
    departed.withdrawn = vec![label("RQ-5")];
    let bindings = recorded(departed.clone());
    assert_eq!(only(&bindings), &departed);
    assert!(bindings.is_departed(key(4)));
    assert_eq!(bindings.resolve(label("RQ-9")), None);
    assert_eq!(bindings.resolve(label("RQ-1")), None);
    assert!(!bindings.is_withdrawn(label("RQ-5")));
}

#[test]
fn a_recorded_binding_keeps_what_a_publish_recorded_in_it() {
    let mut decision = binding(DecisionId::new(key(20)), "ADR-1");
    decision.published_state = Some(StateKind::Accepted);
    decision.published_history = Some(Digest::from_bytes([7; 32]));
    let bindings = recorded(decision.clone());
    assert_eq!(only(&bindings), &decision);
}

/// The small document's bindings with a departed one, a withdrawn label, a decision's published
/// record and a promoted Requirement's label, as a labels file read from disk holds them.
fn varied() -> Document {
    let mut document = small_document();
    let bindings = &mut document.bindings;
    bindings
        .add_withdrawn(RequirementId::new(key(4)), label("RQ-7"))
        .unwrap();
    bindings
        .record_published(
            DecisionId::new(key(20)),
            Some(StateKind::Accepted),
            Some(Digest::from_bytes([3; 32])),
        )
        .unwrap();
    bindings
        .bind(RequirementId::new(key(70)), label("RQ-30"))
        .unwrap();
    bindings.depart(RequirementId::new(key(70))).unwrap();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    let requirement = feature.requirements.remove(0);
    let promoted = loftspec::tree::Feature {
        id: Some(requirement.id.unwrap().promote()),
        slug: crate::common::slug("promoted"),
        name: "Promoted".to_owned(),
        details: loftspec::tree::Details::default(),
        requirements: Vec::new(),
        quality_attributes: Vec::new(),
    };
    document.products[0].feature_groups[0]
        .features
        .push(promoted);
    document
}

#[test]
fn recording_each_binding_a_labels_file_holds_gives_the_bindings_reading_it_gives() {
    let read = codec::read(&codec::write(&varied())).expect("the tree reads");
    assert!(read.bindings.departed().next().is_some());
    let promoted = read
        .bindings
        .iter()
        .find(|binding| binding.element.key() == key(4))
        .expect("the promoted binding");
    assert_eq!(promoted.label, label("RQ-2"));
    assert!(
        matches!(promoted.element, ElementId::Node(node) if node.kind() == loftspec::kind::NodeKind::Feature)
    );
    let mut rebuilt = Bindings::default();
    for binding in read.bindings.iter() {
        rebuilt
            .record(binding.clone())
            .expect("a binding the file held");
    }
    assert_eq!(rebuilt, read.bindings);
}

#[test]
fn a_recorded_binding_of_an_element_bound_already_is_refused_and_changes_nothing() {
    let mut bindings = recorded(binding(RequirementId::new(key(4)), "RQ-2"));
    let before = bindings.clone();
    let mut again = binding(RequirementId::new(key(4)), "RQ-3");
    again.aliases = vec![label("RQ-8")];
    assert_eq!(
        bindings.record(again),
        Err(BindingError::AlreadyBound(
            RequirementId::new(key(4)).into()
        ))
    );
    assert_eq!(bindings, before);
}

#[test]
fn a_recorded_binding_of_a_kind_that_takes_no_label_is_refused_and_changes_nothing() {
    let mut bindings = Bindings::default();
    let section = SectionId::new(key(9));
    assert_eq!(
        bindings.record(binding(section, "RQ-3")),
        Err(BindingError::PrefixMismatch {
            element: section.into(),
            label: label("RQ-3"),
        })
    );
    assert_eq!(bindings, Bindings::default());
}

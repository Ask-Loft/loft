//! Label allocation at publish (F-28, RQ-7, RQ-8, RQ-9, RQ-63): the next label of each kind
//! prefix, in document order, from counters that never rewind.

mod common;

use std::num::NonZeroU64;

use common::{component, criterion, feature, key, label, root, slug, small_document};
use loftspec::codec::{self, SpecPath};
use loftspec::decision::NodeRef;
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::identity::LabelPrefix;
use loftspec::ids::{
    ComponentId, CriterionId, DecisionId, ElementId, FeatureId, NodeId, RequirementId, SectionId,
};
use loftspec::node::Node;
use loftspec::publish::{Allocated, allocate};
use loftspec::tree::Section;

/// The labels allocation gives, as text, in the order given.
fn allocated(document: &mut Document) -> Vec<String> {
    allocate(document)
        .into_iter()
        .map(|allocated| allocated.label.to_string())
        .collect()
}

fn counter(document: &Document, prefix: LabelPrefix) -> u64 {
    document.counters.next(prefix).get()
}

#[test]
fn ac_454_every_node_of_a_kind_that_takes_a_prefix_has_a_binding_after_a_publish() {
    let mut document = small_document();
    allocate(&mut document);
    document.walk(&mut |node| {
        let id = node.id().unwrap();
        assert!(
            document.bindings.label_of(id).is_some(),
            "{id} has no binding"
        );
    });
}

#[test]
fn ac_457_the_publisher_reports_each_allocated_label_with_its_key() {
    let mut document = small_document();
    assert_eq!(
        allocate(&mut document),
        [Allocated {
            label: label("AC-1"),
            element: CriterionId::new(key(5)).into(),
        }]
    );
}

#[test]
fn ac_455_ac_456_a_new_label_is_the_counters_next_value_and_the_counter_advances() {
    let mut document = small_document();
    document
        .counters
        .set(LabelPrefix::Criterion, NonZeroU64::new(7).unwrap());
    assert_eq!(allocated(&mut document), ["AC-7"]);
    assert_eq!(counter(&document, LabelPrefix::Criterion), 8);
}

#[test]
fn ac_25_a_counter_below_a_live_label_is_raised_before_it_gives_one_out() {
    let mut document = small_document();
    document
        .counters
        .set(LabelPrefix::Requirement, NonZeroU64::MIN);
    document.products[0].feature_groups[0].features[0]
        .requirements
        .push(common::requirement(
            key(40),
            "A second requirement.",
            Vec::new(),
        ));
    // The existing Requirement's criterion comes first in document order.
    assert_eq!(allocated(&mut document), ["AC-1", "RQ-3"]);
}

#[test]
fn ac_26_the_label_of_a_deleted_node_is_never_given_out_again() {
    let mut document = small_document();
    // AC-1 to AC-4 were given to criteria since deleted; the counter remembers them.
    document
        .counters
        .set(LabelPrefix::Criterion, NonZeroU64::new(5).unwrap());
    assert_eq!(allocated(&mut document), ["AC-5"]);
}

#[test]
fn ac_722_no_label_that_a_decision_carries_as_a_former_label_is_allocated() {
    let mut document = small_document();
    document.edges.push(Edge::Subject {
        decision: DecisionId::new(key(20)),
        subject: NodeRef::Former(label("AC-3")),
        role: SubjectRole::Affected,
    });
    assert_eq!(allocated(&mut document), ["AC-4"]);
}

#[test]
fn ac_24_labels_are_allocated_in_document_order_parents_before_children() {
    let mut document = small_document();
    let group = &mut document.products[0].feature_groups[0];
    group.features.push(feature(
        key(41),
        "Labels",
        vec![common::requirement(
            key(42),
            "A requirement.",
            vec![criterion(key(43), "A criterion.")],
        )],
    ));
    assert_eq!(allocated(&mut document), ["AC-1", "F-2", "RQ-3", "AC-2"]);
}

#[test]
fn ac_262_root_components_are_visited_in_the_order_of_their_file_names() {
    let mut document = small_document();
    let mut later = root(component(key(44), "Zeta"));
    later.slug = slug("zeta");
    let mut earlier = root(component(key(45), "Alpha"));
    earlier.slug = slug("alpha");
    document.components.extend([later, earlier]);
    assert_eq!(allocated(&mut document), ["AC-1", "CMP-2", "CMP-3"]);
    assert_eq!(
        document.bindings.label_of(ComponentId::new(key(45))),
        Some(label("CMP-2"))
    );
}

#[test]
fn decisions_are_allocated_labels_after_every_node_in_the_order_of_their_file_names() {
    let mut document = small_document();
    let mut second = document.decisions[0].clone();
    second.id = DecisionId::new(key(46));
    second.slug = slug("zeta-decision");
    let mut first = document.decisions[0].clone();
    first.id = DecisionId::new(key(47));
    first.slug = slug("alpha-decision");
    document.decisions.extend([second, first]);
    assert_eq!(allocated(&mut document), ["AC-1", "ADR-2", "ADR-3"]);
    assert_eq!(
        document.bindings.label_of(DecisionId::new(key(47))),
        Some(label("ADR-2"))
    );
}

#[test]
fn ac_691_every_decision_has_a_binding_after_a_publish() {
    let mut document = small_document();
    let mut unbound = document.decisions[0].clone();
    unbound.id = DecisionId::new(key(48));
    unbound.slug = slug("unbound");
    document.decisions.push(unbound);
    allocate(&mut document);
    for decision in &document.decisions {
        assert!(
            document.bindings.label_of(decision.id).is_some(),
            "{}",
            decision.id
        );
    }
}

#[test]
fn ac_243_a_section_takes_no_label() {
    let mut document = small_document();
    document.products[0].details.sections.push(Section {
        id: Some(SectionId::new(key(49))),
        name: "Background".to_owned(),
        body: "Why keys.".to_owned(),
        sections: Vec::new(),
    });
    assert_eq!(allocated(&mut document), ["AC-1"]);
    assert_eq!(document.bindings.label_of(SectionId::new(key(49))), None);
}

#[test]
fn a_label_bound_to_a_section_is_refused_by_the_reader() {
    let mut document = small_document();
    document.products[0].details.sections.push(Section {
        id: Some(SectionId::new(key(49))),
        name: "Background".to_owned(),
        body: "Why keys.".to_owned(),
        sections: Vec::new(),
    });
    let mut files = codec::write(&document);
    let labels = files.get_mut(&SpecPath::new("labels.yaml")).unwrap();
    labels.extend_from_slice(format!("  - key: {}\n    label: P-9\n", key(49)).as_bytes());
    let errors = codec::read(&files).unwrap_err();
    assert!(
        errors.iter().any(|error| error.to_string().contains("P-9")),
        "{errors:?}"
    );
}

/// The small document with its Requirement promoted to a Feature beside F-1: the same key, the
/// Requirement's labels left in the bindings.
fn promoted() -> Document {
    let mut document = small_document();
    let features = &mut document.products[0].feature_groups[0].features;
    features[0].requirements.clear();
    features.push(feature(key(4), "Keys are written", Vec::new()));
    document.edges.retain(|edge| {
        !matches!(edge, Edge::Mentions { target, .. }
            if *target == ElementId::Node(NodeId::Requirement(RequirementId::new(key(4)))))
    });
    document
}

#[test]
fn a_binding_whose_prefix_is_not_its_kinds_is_read_without_error() {
    let document = promoted();
    let mut files = codec::write(&small_document());
    let promoted_files = codec::write(&document);
    files.retain(|path, _| path.as_str() == "labels.yaml");
    let mut tree = promoted_files;
    tree.extend(files);
    let read = codec::read(&tree).unwrap();
    assert_eq!(
        read.bindings.label_of(FeatureId::new(key(4))),
        Some(label("RQ-2"))
    );
}

#[test]
fn ac_223_ac_224_a_promoted_requirement_receives_the_next_f_label_and_keeps_its_own_as_an_alias() {
    let mut document = promoted();
    let element = ElementId::from(FeatureId::new(key(4)));
    assert!(allocate(&mut document).contains(&Allocated {
        label: label("F-2"),
        element,
    }));
    assert_eq!(document.bindings.label_of(element), Some(label("F-2")));
    // AC-28: the alias list only grows; RQ-1 was already an alias of the key.
    assert_eq!(
        document.bindings.aliases_of(element),
        [label("RQ-1"), label("RQ-2")]
    );
}

#[test]
fn ac_225_a_node_whose_kind_is_unchanged_keeps_its_label() {
    let mut document = small_document();
    allocate(&mut document);
    assert_eq!(
        document.bindings.label_of(RequirementId::new(key(4))),
        Some(label("RQ-2"))
    );
}

#[test]
fn a_second_allocation_gives_out_nothing() {
    let mut document = small_document();
    allocate(&mut document);
    assert_eq!(allocate(&mut document), []);
}

#[test]
fn every_kind_takes_its_own_prefix() {
    let mut document = small_document();
    allocate(&mut document);
    document.walk(&mut |node: Node<'_>| {
        let id = node.id().unwrap();
        if let Some(prefix) = ElementId::Node(id).label_prefix() {
            assert_eq!(document.bindings.label_of(id).unwrap().prefix(), prefix);
        }
    });
}

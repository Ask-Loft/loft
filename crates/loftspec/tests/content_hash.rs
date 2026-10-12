//! A node's content view and content hash (D-11 F-3).

mod common;

use common::{component, criterion, feature, fresh_key, key, requirement};
use loftspec::attributes::{
    ConservationKind, QaKind, QualityAggregation, QualityBand, QualityTarget, VerificationFate,
};
use loftspec::canonical::{Domain, canonical_bytes, digest};
use loftspec::hash::ContentHash;
use loftspec::ids::{ConstraintId, FeatureGroupId, QualityAttributeId, SectionId};
use loftspec::names::TagName;
use loftspec::statement::{EarsForm, Statement};
use loftspec::tree::{
    Constraint, Criterion, Details, Feature, FeatureGroup, QualityAttribute, Requirement, Section,
};

fn section(n: u32, name: &str) -> Section {
    Section {
        id: Some(SectionId::new(key(n))),
        name: name.to_owned(),
        body: "Prose.".to_owned(),
        sections: Vec::new(),
    }
}

fn two_requirements(first: &str, second: &str) -> Feature {
    feature(
        key(1),
        "Parent",
        vec![
            requirement(key(2), first, vec![]),
            requirement(key(3), second, vec![]),
        ],
    )
}

#[test]
fn the_content_view_holds_its_fields_in_order() {
    let mut constraint = Constraint {
        id: Some(ConstraintId::new(key(1))),
        name: "The hasher shall use SHA-256.".to_owned(),
        details: Details {
            tags: vec![
                TagName::parse("portable").unwrap(),
                TagName::parse("identity").unwrap(),
                TagName::parse("portable").unwrap(),
            ],
            ..Details::with_body("Only SHA-256.")
        },
        binding: None,
        conservation_kind: Some(ConservationKind::Cap),
        verification_fate: Some(VerificationFate::Proved),
    };
    let notes = section(2, "Notes");
    let notes_hash = notes.content_hash();
    constraint.details.sections = vec![notes];
    let canonical = String::from_utf8(canonical_bytes(&constraint.content_view())).unwrap();
    assert_eq!(
        canonical,
        format!(
            r#"["constraint","The hasher shall use SHA-256.","Only SHA-256.",["identity","portable"],null,null,null,"cap","proved",null,["{notes_hash}"]]"#
        )
    );
}

#[test]
fn equal_views_with_different_keys_have_one_content_hash() {
    let first = criterion(key(1), "A criterion.");
    let second = criterion(fresh_key(1), "A criterion.");
    let unkeyed = Criterion {
        id: None,
        ..first.clone()
    };
    assert_eq!(first.content_hash(), second.content_hash());
    assert_eq!(first.content_hash(), unkeyed.content_hash());
}

#[test]
fn changing_a_body_changes_the_content_hash() {
    let mut node = feature(key(1), "Keys", vec![]);
    let before = node.content_hash();
    node.details.body = Some("Keys address nodes.".to_owned());
    assert_ne!(node.content_hash(), before);
}

#[test]
fn an_empty_body_and_no_body_hash_differently() {
    let mut node = feature(key(1), "Keys", vec![]);
    let without = node.content_hash();
    node.details.body = Some(String::new());
    assert_ne!(node.content_hash(), without);
}

#[test]
fn changing_a_child_changes_the_parent() {
    assert_ne!(
        two_requirements("One.", "Two.").content_hash(),
        two_requirements("One.", "Three.").content_hash()
    );
}

#[test]
fn reordering_children_changes_the_parent_and_no_sibling() {
    let forward = two_requirements("One.", "Two.");
    let mut reversed = forward.clone();
    reversed.requirements.reverse();
    assert_ne!(forward.content_hash(), reversed.content_hash());
    assert_eq!(
        forward.requirements[0].content_hash(),
        reversed.requirements[1].content_hash()
    );
    assert_eq!(
        forward.requirements[1].content_hash(),
        reversed.requirements[0].content_hash()
    );
}

#[test]
fn a_node_hashes_alike_at_any_position_among_its_siblings() {
    let node = requirement(key(9), "Moved.", vec![]);
    let mut first = two_requirements("One.", "Two.");
    first.requirements.insert(0, node.clone());
    let mut last = two_requirements("One.", "Two.");
    last.requirements.push(node.clone());
    assert_eq!(
        first.requirements[0].content_hash(),
        last.requirements[2].content_hash()
    );
}

#[test]
fn tag_order_and_repetition_do_not_change_the_content_hash() {
    let mut first = feature(key(1), "Keys", vec![]);
    first.details.tags = vec![
        TagName::parse("identity").unwrap(),
        TagName::parse("portable").unwrap(),
    ];
    let mut second = first.clone();
    second.details.tags = vec![
        TagName::parse("portable").unwrap(),
        TagName::parse("identity").unwrap(),
        TagName::parse("identity").unwrap(),
    ];
    assert_eq!(first.content_hash(), second.content_hash());
}

#[test]
fn a_statement_enters_as_its_rendered_sentence() {
    let slotted = Requirement {
        statement: Statement::Form(EarsForm::Unwanted {
            r#if: "the manifest is absent".to_owned(),
            system: "the reader".to_owned(),
            shall: "report an error.".to_owned(),
        }),
        ..requirement(key(1), "", vec![])
    };
    let text = requirement(
        key(1),
        "If the manifest is absent, then the reader shall report an error.",
        vec![],
    );
    assert_eq!(slotted.content_hash(), text.content_hash());
}

#[test]
fn a_section_is_hashed_by_the_node_rule() {
    let background = section(9, "Background");
    assert_eq!(
        background.content_hash(),
        digest(Domain::Node, &background.content_view())
    );
}

#[test]
fn sections_follow_the_structural_children() {
    let mut node = feature(key(1), "Keys", vec![requirement(key(2), "One.", vec![])]);
    let notes = section(3, "Notes");
    node.details.sections = vec![notes.clone()];
    let children = node.content_view()[10].clone();
    assert_eq!(children[0], node.requirements[0].content_hash().to_string());
    assert_eq!(children[1], notes.content_hash().to_string());
}

#[test]
fn the_kind_is_content() {
    let feature_node = feature(key(1), "Keys", vec![]);
    let group = FeatureGroup {
        id: Some(FeatureGroupId::new(key(1))),
        slug: common::slug("group"),
        name: "Keys".to_owned(),
        details: Details::default(),
        features: Vec::new(),
        constraints: Vec::new(),
    };
    assert_ne!(feature_node.content_hash(), group.content_hash());
}

fn constraint(n: u32) -> Constraint {
    Constraint {
        id: Some(ConstraintId::new(key(n))),
        name: format!("Constraint {n}."),
        details: Details::default(),
        binding: None,
        conservation_kind: None,
        verification_fate: None,
    }
}

fn quality(n: u32) -> QualityAttribute {
    QualityAttribute {
        id: Some(QualityAttributeId::new(key(n))),
        name: format!("Quality {n}."),
        details: Details::default(),
        kind: QaKind::Latency,
        target: QualityTarget {
            metric: "check.wall-time".to_owned(),
            band: QualityBand::AtMost {
                upper: "2".to_owned(),
            },
            unit: "s".to_owned(),
            aggregation: QualityAggregation::P95,
            scenario: "a canonical tree".to_owned(),
            environment: "the reference machine".to_owned(),
        },
        verification_fate: None,
    }
}

/// The child content hashes a node's content view lists, in order.
fn children(node: &impl ContentHash) -> Vec<String> {
    node.content_view()[10]
        .as_array()
        .unwrap()
        .iter()
        .map(|hash| hash.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_feature_groups_features_precede_its_constraints() {
    let child = feature(key(2), "Keys", vec![]);
    let group = FeatureGroup {
        id: Some(FeatureGroupId::new(key(1))),
        slug: common::slug("group"),
        name: "Identity".to_owned(),
        details: Details::default(),
        features: vec![child.clone()],
        constraints: vec![constraint(3)],
    };
    assert_eq!(
        children(&group),
        vec![
            child.content_hash().to_string(),
            constraint(3).content_hash().to_string(),
        ]
    );
}

#[test]
fn a_features_requirements_precede_its_quality_attributes() {
    let child = requirement(key(2), "The writer shall write a key.", vec![]);
    let mut parent = feature(key(1), "Keys", vec![child.clone()]);
    parent.quality_attributes = vec![quality(3)];
    assert_eq!(
        children(&parent),
        vec![
            child.content_hash().to_string(),
            quality(3).content_hash().to_string(),
        ]
    );
}

#[test]
fn a_components_children_are_grouped_by_kind() {
    let inner = component(key(2), "Inner");
    let mut parent = component(key(1), "Hasher");
    parent.quality_attributes = vec![quality(4)];
    parent.constraints = vec![constraint(3)];
    parent.components = vec![inner.clone()];
    assert_eq!(
        children(&parent),
        vec![
            inner.content_hash().to_string(),
            constraint(3).content_hash().to_string(),
            quality(4).content_hash().to_string(),
        ]
    );
}

#[test]
fn a_nodes_sections_follow_its_other_children() {
    let mut parent = component(key(1), "Hasher");
    parent.details.sections = vec![section(5, "Notes")];
    parent.constraints = vec![constraint(3)];
    assert_eq!(
        children(&parent),
        vec![
            constraint(3).content_hash().to_string(),
            section(5, "Notes").content_hash().to_string(),
        ]
    );
}

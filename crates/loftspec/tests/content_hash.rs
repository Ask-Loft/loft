//! A node's content view and content hash (D-11 F-3).

mod common;

use common::{component, criterion, feature, fresh_key, key, requirement};
use loftspec::attributes::{ConservationKind, VerificationFate};
use loftspec::canonical::{Domain, canonical_bytes, digest};
use loftspec::hash::ContentHash;
use loftspec::ids::{ConstraintId, FeatureGroupId, SectionId};
use loftspec::names::TagName;
use loftspec::statement::{EarsForm, Statement};
use loftspec::tree::{
    ComponentChild, Constraint, Criterion, Details, Feature, FeatureGroup, Requirement, Section,
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
fn ac_34_the_content_view_holds_its_fields_in_order() {
    let mut constraint = Constraint {
        id: Some(ConstraintId::new(key(1))),
        name: "The hasher shall use SHA-256.".to_owned(),
        details: Details {
            tags: vec![
                TagName::new("portable"),
                TagName::new("identity"),
                TagName::new("portable"),
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
            r#"["constraint","The hasher shall use SHA-256.","Only SHA-256.",["identity","portable"],null,null,null,"cap","proved",["{notes_hash}"]]"#
        )
    );
}

#[test]
fn ac_35_and_ac_40_equal_views_with_different_keys_have_one_content_hash() {
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
fn ac_41_changing_a_body_changes_the_content_hash() {
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
fn ac_42_changing_a_child_changes_the_parent() {
    assert_ne!(
        two_requirements("One.", "Two.").content_hash(),
        two_requirements("One.", "Three.").content_hash()
    );
}

#[test]
fn ac_43_and_ac_44_reordering_children_changes_the_parent_and_no_sibling() {
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
fn ac_37_a_node_hashes_alike_at_any_position_among_its_siblings() {
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
    first.details.tags = vec![TagName::new("identity"), TagName::new("portable")];
    let mut second = first.clone();
    second.details.tags = vec![
        TagName::new("portable"),
        TagName::new("identity"),
        TagName::new("identity"),
    ];
    assert_eq!(first.content_hash(), second.content_hash());
}

#[test]
fn ac_242_a_statement_enters_as_its_rendered_sentence() {
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
fn ac_158_a_section_is_hashed_by_the_node_rule() {
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
    let children = node.content_view()[9].clone();
    assert_eq!(children[0], node.requirements[0].content_hash().to_string());
    assert_eq!(children[1], notes.content_hash().to_string());
}

#[test]
fn the_kind_is_content() {
    let feature_node = feature(key(1), "Keys", vec![]);
    let group = FeatureGroup {
        id: Some(FeatureGroupId::new(key(1))),
        name: "Keys".to_owned(),
        details: Details::default(),
        features: Vec::new(),
    };
    assert_ne!(feature_node.content_hash(), group.content_hash());
}

#[test]
fn a_component_keeps_one_ordered_list_of_mixed_children() {
    let constraint = |n: u32| {
        ComponentChild::Constraint(Constraint {
            id: Some(ConstraintId::new(key(n))),
            name: format!("Constraint {n}."),
            details: Details::default(),
            binding: None,
            conservation_kind: None,
            verification_fate: None,
        })
    };
    let mut first = component(key(1), "Hasher");
    first.children = vec![
        constraint(2),
        ComponentChild::Component(component(key(3), "Inner")),
        constraint(4),
    ];
    let mut second = first.clone();
    second.children.swap(0, 1);
    assert_ne!(first.content_hash(), second.content_hash());
}

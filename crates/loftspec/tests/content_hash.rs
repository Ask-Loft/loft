//! A node's content view and content hash (D-11 F-3).

mod common;

use common::{key, named};
use loftspec::attributes::{Attributes, ConservationKind, VerificationFate};
use loftspec::canonical::{Domain, canonical_bytes, digest};
use loftspec::hash::{content_hash, node_view};
use loftspec::kind::NodeKind;
use loftspec::model::{Node, Title};
use loftspec::statement::{EarsForm, Statement};

fn parent_with(children: Vec<Node>) -> Node {
    let mut parent = named(1, NodeKind::Feature, "Parent");
    parent.children = children;
    parent
}

#[test]
fn ac_34_the_content_view_holds_its_fields_in_order() {
    let mut node = named(1, NodeKind::Constraint, "The hasher shall use SHA-256.");
    node.body = Some("Only SHA-256.".to_owned());
    node.tags = vec![
        "portable".to_owned(),
        "identity".to_owned(),
        "portable".to_owned(),
    ];
    node.attributes = Attributes {
        conservation_kind: Some(ConservationKind::Cap),
        verification_fate: Some(VerificationFate::Proved),
        ..Attributes::default()
    };
    let child = named(2, NodeKind::Section, "Notes");
    let child_hash = content_hash(&child);
    node.children = vec![child];
    let canonical = String::from_utf8(canonical_bytes(&node_view(&node))).unwrap();
    assert_eq!(
        canonical,
        format!(
            r#"["constraint","The hasher shall use SHA-256.","Only SHA-256.",["identity","portable"],null,null,null,"cap","proved",["{child_hash}"]]"#
        )
    );
}

#[test]
fn ac_35_and_ac_40_equal_views_with_different_keys_have_one_content_hash() {
    let first = named(1, NodeKind::Criterion, "A criterion.");
    let second = named(2, NodeKind::Criterion, "A criterion.");
    let unkeyed = Node::named(None, NodeKind::Criterion, "A criterion.");
    assert_eq!(content_hash(&first), content_hash(&second));
    assert_eq!(content_hash(&first), content_hash(&unkeyed));
}

#[test]
fn ac_41_changing_a_body_changes_the_content_hash() {
    let mut node = named(1, NodeKind::Feature, "Keys");
    let before = content_hash(&node);
    node.body = Some("Keys address nodes.".to_owned());
    assert_ne!(content_hash(&node), before);
}

#[test]
fn an_empty_body_and_no_body_hash_differently() {
    let mut node = named(1, NodeKind::Feature, "Keys");
    let without = content_hash(&node);
    node.body = Some(String::new());
    assert_ne!(content_hash(&node), without);
}

#[test]
fn ac_42_changing_a_child_changes_the_parent() {
    let before = content_hash(&parent_with(vec![named(2, NodeKind::Requirement, "One.")]));
    let after = content_hash(&parent_with(vec![named(2, NodeKind::Requirement, "Two.")]));
    assert_ne!(before, after);
}

#[test]
fn ac_43_and_ac_44_reordering_children_changes_the_parent_and_no_sibling() {
    let one = named(2, NodeKind::Requirement, "One.");
    let two = named(3, NodeKind::Requirement, "Two.");
    let (one_hash, two_hash) = (content_hash(&one), content_hash(&two));
    let forward = parent_with(vec![one.clone(), two.clone()]);
    let reversed = parent_with(vec![two.clone(), one.clone()]);
    assert_ne!(content_hash(&forward), content_hash(&reversed));
    assert_eq!(content_hash(&reversed.children[1]), one_hash);
    assert_eq!(content_hash(&reversed.children[0]), two_hash);
}

#[test]
fn ac_37_a_node_hashes_alike_at_any_position_among_its_siblings() {
    let node = named(2, NodeKind::Requirement, "One.");
    let first = parent_with(vec![node.clone(), named(3, NodeKind::Requirement, "Two.")]);
    let second = parent_with(vec![named(3, NodeKind::Requirement, "Two."), node.clone()]);
    assert_eq!(
        content_hash(&first.children[0]),
        content_hash(&second.children[1])
    );
}

#[test]
fn tag_order_and_repetition_do_not_change_the_content_hash() {
    let mut first = named(1, NodeKind::Feature, "Keys");
    first.tags = vec!["identity".to_owned(), "portable".to_owned()];
    let mut second = first.clone();
    second.tags = vec![
        "portable".to_owned(),
        "identity".to_owned(),
        "identity".to_owned(),
    ];
    assert_eq!(content_hash(&first), content_hash(&second));
}

#[test]
fn ac_242_a_statement_enters_as_its_rendered_sentence() {
    let mut slotted = named(1, NodeKind::Requirement, "");
    slotted.title = Title::Statement(Statement::Form(EarsForm::Unwanted {
        r#if: "the manifest is absent".to_owned(),
        system: "the reader".to_owned(),
        shall: "report an error.".to_owned(),
    }));
    let mut text = slotted.clone();
    text.title = Title::Statement(Statement::Text(
        "If the manifest is absent, then the reader shall report an error.".to_owned(),
    ));
    assert_eq!(content_hash(&slotted), content_hash(&text));
}

#[test]
fn ac_158_a_section_is_hashed_by_the_node_rule() {
    let section = named(9, NodeKind::Section, "Background");
    assert_eq!(
        content_hash(&section),
        digest(Domain::Node, &node_view(&section))
    );
}

#[test]
fn the_kind_is_content() {
    let feature = named(1, NodeKind::Feature, "Keys");
    let group = named(1, NodeKind::FeatureGroup, "Keys");
    assert_ne!(content_hash(&feature), content_hash(&group));
    assert_eq!(key(1), feature.key.unwrap());
}

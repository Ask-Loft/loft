//! The containment table (F-17) and the canonical traversal of the tree (RQ-160).

use crate::common::small_document;
use loftspec::kind::NodeKind;
use loftspec::node::Node;

#[test]
fn each_kind_holds_only_the_kinds_the_profile_allows() {
    use NodeKind::{
        Component, Constraint, Criterion, Feature, FeatureGroup, Product, QualityAttribute,
        Requirement,
    };
    assert!(Product.may_contain(FeatureGroup));
    assert!(!Product.may_contain(Feature));
    assert!(FeatureGroup.may_contain(Constraint));
    assert!(!FeatureGroup.may_contain(QualityAttribute));
    assert!(Feature.may_contain(QualityAttribute));
    assert!(Component.may_contain(Component));
    assert!(!Requirement.may_contain(Requirement));
    for leaf in [Criterion, Constraint, QualityAttribute] {
        assert!(leaf.child_kinds().is_empty(), "{leaf}");
    }
}

#[test]
fn every_kind_may_hold_sections() {
    for kind in NodeKind::ALL {
        assert!(kind.may_contain(NodeKind::Section), "{kind}");
    }
}

#[test]
fn children_follow_the_table_then_the_sections() {
    let document = small_document();
    let mut seen = 0;
    for root in &document.products {
        Node::from(root).walk(&mut |node| {
            seen += 1;
            let kinds: Vec<NodeKind> = node.children().iter().map(|child| child.kind()).collect();
            let structural = kinds.iter().take_while(|kind| **kind != NodeKind::Section);
            let mut order = structural.map(|kind| {
                node.kind()
                    .child_kinds()
                    .iter()
                    .position(|allowed| allowed == kind)
                    .expect("a child kind the table allows")
            });
            let mut last = 0;
            assert!(order.all(|position| {
                let ascending = position >= last;
                last = position;
                ascending
            }));
            let first_section = kinds.iter().position(|kind| *kind == NodeKind::Section);
            if let Some(first) = first_section {
                assert!(kinds[first..].iter().all(|kind| *kind == NodeKind::Section));
            }
        });
    }
    assert!(seen > 1);
}

#[test]
fn walk_visits_parents_before_children() {
    let document = small_document();
    let product = &document.products[0];
    let mut kinds = Vec::new();
    Node::from(product).walk(&mut |node| kinds.push(node.kind()));
    assert_eq!(kinds[0], NodeKind::Product);
    assert_eq!(kinds[1], NodeKind::FeatureGroup);
    assert!(kinds.contains(&NodeKind::Criterion));
}

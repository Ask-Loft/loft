//! The edges each node of the review packet writes (The review packet), over the small document
//! of `review`: F-1 (3) realizes CMP-1 (6), which is scoped to P-1 (1).

use crate::common::{component, key, product};
use crate::review::{criterion_mut, ns, packed, unit};
use loftspec::document::{Document, Edge};
use loftspec::identity::Key;
use loftspec::ids::{ComponentId, FeatureId, ProductId};
use loftspec::review::{Change, Fact};

use crate::common::small_document;

fn realized(fact: &Fact) -> Vec<Key> {
    fact.realizes.iter().map(|id| id.key()).collect()
}

fn scoped(fact: &Fact) -> Vec<Key> {
    fact.scoped_to.iter().map(|id| id.key()).collect()
}

/// The small document with criterion 5 renamed, so that F-1's unit is reviewed against a base.
fn with_the_criterion_renamed() -> Document {
    let mut document = small_document();
    "Every node record carries a key.".clone_into(&mut criterion_mut(&mut document).name);
    document
}

#[test]
fn each_feature_carries_the_components_it_realizes_in_key_order() {
    let mut document = small_document();
    let part = component(key(40), "Key minting");
    document.components[0].component.components.push(part);
    document.edges.insert(
        0,
        Edge::Realizes {
            feature: FeatureId::new(key(3)),
            component: ComponentId::new(key(40)),
        },
    );
    let packet = packed(&document, None, None);
    let feature = unit(&packet, 3);
    assert_eq!(realized(&feature.scope.node), ns(&[6, 40]));
    assert_eq!(realized(&feature.requirements[0].criteria[0]), ns(&[]));
}

#[test]
fn each_component_carries_the_products_it_is_scoped_to_in_key_order_each_once() {
    let mut document = small_document();
    document.products.push(product(key(30), "Second product"));
    for product in [1, 30] {
        document.edges.insert(
            0,
            Edge::ScopedTo {
                component: ComponentId::new(key(6)),
                product: ProductId::new(key(product)),
            },
        );
    }
    let packet = packed(&document, None, None);
    let feature = unit(&packet, 3);
    assert_eq!(scoped(&feature.realizes[0].component.node), ns(&[1, 30]));
    assert_eq!(scoped(&feature.scope.node), ns(&[]));
}

#[test]
fn a_scope_whose_realizes_alone_differ_is_changed() {
    let mut document = with_the_criterion_renamed();
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Realizes { .. }));
    let packet = packed(&document, Some(&small_document()), None);
    let scope = &unit(&packet, 3).scope.node;
    assert_eq!(scope.change, Some(Change::Changed));
    assert_eq!(realized(scope), ns(&[]));
}

#[test]
fn a_realized_component_whose_scoped_to_alone_differs_is_changed() {
    let mut document = with_the_criterion_renamed();
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::ScopedTo { .. }));
    let packet = packed(&document, Some(&small_document()), None);
    let feature = unit(&packet, 3);
    assert_eq!(feature.scope.node.change, None);
    let realized = &feature.realizes[0].component.node;
    assert_eq!(realized.change, Some(Change::Changed));
    assert_eq!(scoped(realized), ns(&[]));
}

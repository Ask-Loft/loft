//! Editing a document held in memory (Library entry points): inserting a subtree under a parent
//! at a position, and moving a node to another parent or position, keeping its key and every
//! node beneath it.

use crate::common::editing::{
    GROUP, INNER, KEYS, LABELS, NESTED, ROOT, SLUGS, STORAGE, beneath, changes, editable,
    feature_id, features, number,
};
use crate::common::{key, product};
use loftspec::document::{Document, Edge};
use loftspec::edit::{self, Found, Parent, Refusal, Subtree};
use loftspec::ids::{ComponentId, FeatureGroupId, NodeId, ProductId};
use loftspec::node::Node;

fn group(n: u32) -> Parent {
    Parent::Node(FeatureGroupId::new(key(n)).into())
}

fn component(n: u32) -> NodeId {
    ComponentId::new(key(n)).into()
}

fn found_node(document: &Document, n: u32) -> Node<'_> {
    match edit::find(document, key(n)) {
        Some(Found::Node(node)) => node,
        other => panic!("key {n} found {other:?}"),
    }
}

/// The edges as a sorted list of their debug forms, whose order means nothing.
fn edge_set(document: &Document) -> Vec<String> {
    let mut edges: Vec<String> = document
        .edges
        .iter()
        .map(|edge: &Edge| format!("{edge:?}"))
        .collect();
    edges.sort();
    edges
}

/// The editable document with the Feature `n` detached, and that Feature.
fn without(n: u32) -> (Document, Subtree) {
    let mut document = editable();
    let subtree = edit::detach(&mut document, feature_id(n)).unwrap();
    (document, subtree)
}

#[test]
fn the_inserted_node_is_at_the_given_position_in_its_parents_list() {
    let (mut document, subtree) = without(LABELS);
    edit::insert(&mut document, group(GROUP), 0, subtree).unwrap();
    assert_eq!(features(&document, 0), [LABELS, KEYS, SLUGS]);
}

#[test]
fn the_nodes_that_followed_the_position_follow_the_inserted_node_in_their_order() {
    let (mut document, subtree) = without(KEYS);
    assert_eq!(features(&document, 0), [LABELS, SLUGS]);
    edit::insert(&mut document, group(GROUP), 1, subtree).unwrap();
    assert_eq!(features(&document, 0), [LABELS, KEYS, SLUGS]);
}

#[test]
fn a_position_equal_to_the_length_of_the_list_appends_the_node() {
    let (mut document, subtree) = without(KEYS);
    edit::insert(&mut document, group(GROUP), 2, subtree).unwrap();
    assert_eq!(features(&document, 0), [LABELS, SLUGS, KEYS]);
}

#[test]
fn the_parent_of_an_inserted_product_or_root_component_is_the_documents_root() {
    let mut document = editable();
    let shelf = Subtree::new(product(key(200), "Shelf"));
    edit::insert(&mut document, Parent::Root, 0, shelf).unwrap();
    let root = edit::detach(&mut document, component(ROOT)).unwrap();
    edit::insert(&mut document, Parent::Root, 0, root).unwrap();
    let products: Vec<u32> = document
        .products
        .iter()
        .map(|p| number(p.id.unwrap().key()))
        .collect();
    assert_eq!(products, [200, 1]);
    assert_eq!(
        document.components[0].component.id,
        Some(ComponentId::new(key(ROOT)))
    );
    assert_eq!(document.components[0].slug.as_str(), "document-model");
    assert_eq!(edge_set(&document), edge_set(&editable()));
}

#[test]
fn every_node_beneath_the_inserted_node_is_inserted_with_it() {
    let before = editable();
    let numbers = beneath(found_node(&before, KEYS));
    let (mut document, subtree) = without(KEYS);
    edit::insert(&mut document, group(STORAGE), 0, subtree).unwrap();
    assert_eq!(beneath(found_node(&document, KEYS)), numbers);
    assert_eq!(edge_set(&document), edge_set(&before));
}

#[test]
fn every_inserted_node_keeps_its_key() {
    let (mut document, subtree) = without(KEYS);
    let ids = subtree.ids();
    edit::insert(&mut document, group(STORAGE), 0, subtree).unwrap();
    let node = found_node(&document, KEYS);
    let mut inserted = Vec::new();
    node.walk(&mut |each| inserted.extend(each.id()));
    assert_eq!(inserted, ids);
}

#[test]
fn the_moved_node_is_at_the_given_position_in_its_new_parents_list() {
    let mut document = editable();
    edit::move_node(&mut document, feature_id(LABELS), group(STORAGE), 0).unwrap();
    assert_eq!(features(&document, 0), [KEYS, SLUGS]);
    assert_eq!(features(&document, 1), [LABELS]);
    edit::move_node(&mut document, feature_id(KEYS), group(STORAGE), 1).unwrap();
    assert_eq!(features(&document, 1), [LABELS, KEYS]);
}

#[test]
fn the_moved_node_keeps_its_key() {
    let mut document = editable();
    edit::move_node(
        &mut document,
        component(INNER),
        Parent::Node(component(ROOT)),
        0,
    )
    .unwrap();
    let root = &document.components[0].component;
    assert_eq!(root.components[0].id, Some(ComponentId::new(key(INNER))));
    assert_eq!(root.components[1].id, Some(ComponentId::new(key(NESTED))));
    assert_eq!(root.components[1].components, []);
}

#[test]
fn every_node_beneath_the_moved_node_stays_beneath_it_and_keeps_its_key() {
    let before = editable();
    let numbers = beneath(found_node(&before, KEYS));
    let mut document = before.clone();
    edit::move_node(&mut document, feature_id(KEYS), group(STORAGE), 0).unwrap();
    assert_eq!(beneath(found_node(&document, KEYS)), numbers);
}

#[test]
fn a_position_under_the_nodes_own_parent_counts_the_list_without_it() {
    let mut document = editable();
    let refused = edit::move_node(&mut document, feature_id(KEYS), group(GROUP), 3);
    assert_eq!(
        refused,
        Err(Refusal::Position {
            position: 3,
            length: 2
        })
    );
    edit::move_node(&mut document, feature_id(KEYS), group(GROUP), 2).unwrap();
    assert_eq!(features(&document, 0), [LABELS, SLUGS, KEYS]);
    edit::move_node(&mut document, feature_id(KEYS), group(GROUP), 0).unwrap();
    assert_eq!(features(&document, 0), [KEYS, LABELS, SLUGS]);
}

#[test]
fn a_move_changes_no_other_node_and_no_decision() {
    let before = editable();
    let mut document = before.clone();
    edit::move_node(&mut document, feature_id(LABELS), group(STORAGE), 0).unwrap();
    let changes = changes(&before, &document);
    assert_eq!(changes.moved, [LABELS]);
    assert!(changes.added.is_empty() && changes.changed.is_empty() && changes.removed.is_empty());
    assert_eq!(document.decisions, before.decisions);
    assert_eq!(document.edges, before.edges);
}

#[test]
fn a_product_moved_at_the_root_keeps_its_place_among_the_products() {
    let mut document = editable();
    let shelf = Subtree::new(product(key(200), "Shelf"));
    edit::insert(&mut document, Parent::Root, 1, shelf).unwrap();
    let example = ProductId::new(key(1)).into();
    edit::move_node(&mut document, example, Parent::Root, 1).unwrap();
    let products: Vec<u32> = document
        .products
        .iter()
        .map(|p| number(p.id.unwrap().key()))
        .collect();
    assert_eq!(products, [200, 1]);
}

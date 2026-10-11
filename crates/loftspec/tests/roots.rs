//! The edges, registry, decisions, content, binding and state roots (D-11 F-4, F-5).

mod common;

use common::{edge, key, label, named, small_document};
use loftspec::decision::{Decision, DecisionState, DriverPayload};
use loftspec::hash::{HashError, content_hash, hash_document};
use loftspec::identity::Key;
use loftspec::kind::NodeKind;
use loftspec::model::{DerivedFrom, Document, EdgeKind, Node};

fn roots(document: &Document) -> loftspec::hash::Roots {
    hash_document(document).expect("the document hashes").roots
}

/// The same document with every key replaced by a fresh one, edges and bindings following.
fn rekeyed(document: &Document) -> Document {
    let mut fresh = document.clone();
    let mut mapping = std::collections::BTreeMap::new();
    let mut remap = |key: &mut Key| *key = *mapping.entry(*key).or_insert_with(Key::mint);
    for node in fresh.products.iter_mut().chain(&mut fresh.components) {
        remap_tree(node, &mut remap);
    }
    for edge in &mut fresh.edges {
        remap(&mut edge.source);
        remap(&mut edge.target);
    }
    for decision in &mut fresh.decisions {
        remap_decision(decision, &mut remap);
    }
    for entry in fresh.tags.iter_mut().chain(&mut fresh.categories) {
        remap(&mut entry.key);
    }
    fresh.bindings = loftspec::bindings::Bindings::default();
    fresh
}

fn remap_tree(node: &mut Node, remap: &mut dyn FnMut(&mut Key)) {
    if let Some(key) = node.key.as_mut() {
        remap(key);
    }
    for child in &mut node.children {
        remap_tree(child, remap);
    }
}

fn remap_decision(decision: &mut Decision, remap: &mut dyn FnMut(&mut Key)) {
    remap(&mut decision.key);
    for option in &mut decision.options {
        remap(&mut option.key);
    }
    for driver in &mut decision.drivers {
        remap(&mut driver.key);
        if let DriverPayload::Subject { subject } = &mut driver.payload {
            remap(subject);
        }
    }
    for consequence in &mut decision.consequences {
        remap(&mut consequence.key);
        if let Some(subject) = consequence.subject.as_mut() {
            remap(subject);
        }
    }
    for assessment in &mut decision.assessments {
        remap(&mut assessment.option);
        remap(&mut assessment.driver);
    }
    if let DecisionState::Accepted {
        selected_option, ..
    } = &mut decision.state
    {
        remap(selected_option);
    }
}

#[test]
fn ac_49_and_ac_52_an_edge_enters_by_its_endpoints_content_so_rekeying_leaves_the_edges_root() {
    let document = small_document();
    let fresh = rekeyed(&document);
    assert_eq!(roots(&document).edges, roots(&fresh).edges);
}

#[test]
fn ac_54_the_same_document_with_fresh_keys_has_the_same_content_root() {
    let document = small_document();
    assert_eq!(roots(&document).content, roots(&rekeyed(&document)).content);
}

#[test]
fn ac_50_two_identical_edges_between_duplicate_nodes_both_count() {
    let mut document = small_document();
    document
        .components
        .push(named(40, NodeKind::Component, "Twin"));
    document
        .components
        .push(named(41, NodeKind::Component, "Twin"));
    document.edges.push(edge(3, EdgeKind::Realizes, 40, None));
    let one = roots(&document).edges;
    document.edges.push(edge(3, EdgeKind::Realizes, 41, None));
    assert_ne!(roots(&document).edges, one);
}

#[test]
fn ac_51_and_ac_252_adding_an_edge_changes_the_edges_root_and_the_content_root() {
    let mut document = small_document();
    let before = roots(&document);
    document.edges.push(edge(4, EdgeKind::Mentions, 6, None));
    let after = roots(&document);
    assert_ne!(after.edges, before.edges);
    assert_ne!(after.content, before.content);
}

#[test]
fn edge_order_does_not_change_the_edges_root() {
    let mut document = small_document();
    let before = roots(&document).edges;
    document.edges.reverse();
    assert_eq!(roots(&document).edges, before);
}

#[test]
fn ac_53_ac_55_and_ac_59_relabelling_moves_the_binding_root_and_nothing_else() {
    let mut document = small_document();
    let before = roots(&document);
    document.bindings.relabel(key(3), label("F-9")).unwrap();
    let after = roots(&document);
    assert_eq!(after.edges, before.edges);
    assert_eq!(after.content, before.content);
    assert_ne!(after.bindings, before.bindings);
    assert_ne!(after.state, before.state);
}

#[test]
fn ac_56_reordering_the_products_changes_the_content_root() {
    let mut document = small_document();
    document
        .products
        .push(named(50, NodeKind::Product, "Second Product"));
    let before = roots(&document).content;
    document.products.reverse();
    assert_ne!(roots(&document).content, before);
}

#[test]
fn ac_57_and_ac_254_root_components_are_sorted_by_content_hash() {
    let mut document = small_document();
    document
        .components
        .push(named(51, NodeKind::Component, "Second Component"));
    let hashes = hash_document(&document).unwrap();
    let before = hashes.roots.content;
    let mut sorted = hashes.components.clone();
    sorted.sort();
    assert_eq!(hashes.components, sorted);
    document.components.reverse();
    assert_eq!(roots(&document).content, before);
}

#[test]
fn ac_58_and_ac_216_changing_a_registry_entry_changes_its_root_and_the_content_root() {
    let mut document = small_document();
    let before = roots(&document);
    document.tags[0].definition.push_str(" Always.");
    let after = roots(&document);
    assert_ne!(after.tags, before.tags);
    assert_eq!(after.categories, before.categories);
    assert_ne!(after.content, before.content);
}

#[test]
fn ac_214_ac_215_and_ac_217_registry_roots_are_over_entries_sorted_by_name() {
    let mut document = small_document();
    let before = roots(&document);
    document.tags.reverse();
    document.categories.push(loftspec::model::RegistryEntry {
        key: key(33),
        name: "a.first".to_owned(),
        definition: "Sorts first.".to_owned(),
    });
    let with_category = roots(&document).categories;
    document.categories.reverse();
    let after = roots(&document);
    assert_eq!(after.tags, before.tags);
    assert_eq!(after.categories, with_category);
    assert_ne!(after.categories, before.categories);
}

#[test]
fn the_two_registries_have_distinct_roots_even_when_equal() {
    let mut document = small_document();
    document.categories = document.tags.clone();
    let roots = roots(&document);
    assert_ne!(roots.tags, roots.categories);
}

#[test]
fn ac_219_and_ac_220_fresh_element_keys_leave_the_decision_view_hash() {
    let document = small_document();
    let fresh = rekeyed(&document);
    let before = hash_document(&document).unwrap();
    let after = hash_document(&fresh).unwrap();
    assert_eq!(
        before.decisions[&key(20)],
        after.decisions[&fresh.decisions[0].key]
    );
    assert_eq!(before.roots.decisions, after.roots.decisions);
}

#[test]
fn ac_221_and_ac_253_changing_an_outcome_changes_the_decisions_root_and_the_content_root() {
    let mut document = small_document();
    let before = roots(&document);
    document.decisions[0].state = DecisionState::Accepted {
        selected_option: key(23),
        outcome: "New keys are UUID version 7, minted by the formatter.".to_owned(),
    };
    let after = roots(&document);
    assert_ne!(after.decisions, before.decisions);
    assert_ne!(after.content, before.content);
}

#[test]
fn ac_222_decision_order_leaves_the_decisions_root() {
    let mut document = small_document();
    let mut second = document.decisions[0].clone();
    second.key = key(60);
    second.title = "A second decision".to_owned();
    document.decisions.push(second);
    let before = roots(&document).decisions;
    document.decisions.reverse();
    assert_eq!(roots(&document).decisions, before);
}

#[test]
fn assessment_order_leaves_the_decision_view_hash() {
    let mut document = small_document();
    let before = hash_document(&document).unwrap().decisions[&key(20)];
    document.decisions[0].assessments.reverse();
    assert_eq!(
        hash_document(&document).unwrap().decisions[&key(20)],
        before
    );
}

#[test]
fn ac_240_a_bindings_content_hash_is_computed_from_the_tree() {
    let mut document = small_document();
    let before = roots(&document).bindings;
    document.products[0].children[0].children[0].body = Some("Changed.".to_owned());
    assert_ne!(
        roots(&document).bindings,
        before,
        "F-1's content hash enters its binding"
    );
}

#[test]
fn ac_62_and_ac_63_every_state_view_field_moves_the_state_root() {
    let document = small_document();
    let before = roots(&document).state;
    let mut rekeyed_document = document.clone();
    rekeyed_document.key = key(99);
    assert_ne!(roots(&rekeyed_document).state, before, "document key");
    let mut derived = document.clone();
    derived.derived_from = Some(DerivedFrom {
        authored_state_id: key(98),
        content_address: "3823fe93df167a36ae3b73fb0d77a4630a5e42b40b639f6d45fd46909caca312"
            .to_owned(),
        publish_sequence: 2,
    });
    let derived_root = roots(&derived).state;
    assert_ne!(derived_root, before, "derived-from record");
    derived.derived_from.as_mut().unwrap().publish_sequence = 3;
    assert_ne!(
        roots(&derived).state,
        derived_root,
        "derived-from publish sequence"
    );
}

#[test]
fn ac_64_equal_state_views_have_the_same_state_root() {
    assert_eq!(
        roots(&small_document()).state,
        roots(&small_document()).state
    );
}

#[test]
fn an_edge_to_an_unknown_key_is_refused() {
    let mut document = small_document();
    document.edges.push(edge(3, EdgeKind::Realizes, 77, None));
    assert_eq!(
        hash_document(&document).unwrap_err(),
        HashError::UnknownEdgeEndpoint(key(77))
    );
}

#[test]
fn a_binding_of_an_unknown_key_is_refused() {
    let mut document = small_document();
    document.bindings.bind(key(77), label("RQ-77")).unwrap();
    assert_eq!(
        hash_document(&document).unwrap_err(),
        HashError::UnknownBindingKey(key(77))
    );
}

#[test]
fn a_key_carried_twice_is_refused() {
    let mut document = small_document();
    document
        .components
        .push(named(6, NodeKind::Component, "Another"));
    assert_eq!(
        hash_document(&document).unwrap_err(),
        HashError::DuplicateKey(key(6))
    );
}

#[test]
fn the_tree_hashes_are_the_node_hashes() {
    let document = small_document();
    let hashes = hash_document(&document).unwrap();
    assert_eq!(hashes.products, [content_hash(&document.products[0])]);
    assert_eq!(
        hashes.nodes[&key(5)],
        content_hash(&document.products[0].children[0].children[0].children[0].children[0])
    );
}

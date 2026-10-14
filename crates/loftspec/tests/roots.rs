//! The edges, registry, decisions, content, binding and state roots (F-4, F-5).

use crate::common::{
    component, fresh_key, key, label, product, root, small_document, small_document_with,
};
use loftspec::canonical::{Digest, Domain, digest};
use loftspec::decision::DecisionState;
use loftspec::document::{CategoryEntry, DerivedFrom, Document, Edge};
use loftspec::hash::{ContentHash, Roots, hash_document, hash_indexed_with_views};
use loftspec::ids::{
    AuthoredStateId, CategoryId, ComponentId, DecisionId, DocumentId, ElementId, FeatureId,
    OptionId, RequirementId,
};
use loftspec::index::IntegrityError;
use loftspec::names::CategoryName;
use serde_json::json;

fn roots(document: &Document) -> Roots {
    hash_document(document).expect("the document hashes").roots
}

fn realizes(feature: u32, component: u32) -> Edge {
    Edge::Realizes {
        feature: FeatureId::new(key(feature)),
        component: ComponentId::new(key(component)),
    }
}

#[test]
fn an_edge_enters_by_its_endpoints_content_so_rekeying_leaves_the_edges_root() {
    assert_eq!(
        roots(&small_document()).edges,
        roots(&small_document_with(&fresh_key)).edges
    );
}

#[test]
fn the_same_document_with_fresh_keys_has_the_same_content_root() {
    assert_eq!(
        roots(&small_document()).content,
        roots(&small_document_with(&fresh_key)).content
    );
}

#[test]
fn two_identical_edges_between_duplicate_nodes_both_count() {
    let mut document = small_document();
    document.components.push(root(component(key(40), "Twin")));
    document.components.push(root(component(key(41), "Twin")));
    document.edges.push(realizes(3, 40));
    let one = roots(&document).edges;
    document.edges.push(realizes(3, 41));
    assert_ne!(roots(&document).edges, one);
}

#[test]
fn adding_an_edge_changes_the_edges_root_and_the_content_root() {
    let mut document = small_document();
    let before = roots(&document);
    document.edges.push(Edge::Mentions {
        source: RequirementId::new(key(4)).into(),
        target: ComponentId::new(key(6)).into(),
    });
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
fn relabelling_moves_the_binding_root_and_nothing_else() {
    let mut document = small_document();
    let before = roots(&document);
    document
        .bindings
        .relabel(FeatureId::new(key(3)), label("F-9"))
        .unwrap();
    let after = roots(&document);
    assert_eq!(after.edges, before.edges);
    assert_eq!(after.content, before.content);
    assert_ne!(after.bindings, before.bindings);
    assert_ne!(after.state, before.state);
}

#[test]
fn reordering_the_products_changes_the_content_root() {
    let mut document = small_document();
    document.products.push(product(key(50), "Second Product"));
    let before = roots(&document).content;
    document.products.reverse();
    assert_ne!(roots(&document).content, before);
}

#[test]
fn root_components_are_sorted_by_content_hash() {
    let mut document = small_document();
    document
        .components
        .push(root(component(key(51), "Second Component")));
    let hashes = hash_document(&document).unwrap();
    let mut sorted: Vec<Digest> = document
        .components
        .iter()
        .map(ContentHash::content_hash)
        .collect();
    sorted.sort();
    assert_eq!(hashes.components, sorted);
    document.components.reverse();
    assert_eq!(roots(&document).content, hashes.roots.content);
}

#[test]
fn changing_a_registry_entry_changes_its_root_and_the_content_root() {
    let mut document = small_document();
    let before = roots(&document);
    document.tags[0].definition.push_str(" Always.");
    let after = roots(&document);
    assert_ne!(after.tags, before.tags);
    assert_eq!(after.categories, before.categories);
    assert_ne!(after.content, before.content);
}

#[test]
fn registry_roots_are_over_entries_sorted_by_name() {
    let mut document = small_document();
    let before = roots(&document);
    document.tags.reverse();
    document.categories.push(CategoryEntry {
        id: CategoryId::new(key(33)),
        name: CategoryName::parse("a.first").unwrap(),
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
    document.categories = document
        .tags
        .iter()
        .zip(40..)
        .map(|(tag, n)| CategoryEntry {
            id: CategoryId::new(key(n)),
            name: CategoryName::parse(tag.name.as_str()).unwrap(),
            definition: tag.definition.clone(),
        })
        .collect();
    document.tags.clear();
    let mut twin = document.clone();
    twin.tags = small_document().tags;
    let roots = roots(&twin);
    assert_ne!(roots.tags, roots.categories);
}

#[test]
fn fresh_element_keys_leave_the_decision_view_hash() {
    let before = hash_document(&small_document()).unwrap();
    let after = hash_document(&small_document_with(&fresh_key)).unwrap();
    assert_eq!(
        before.decisions[&DecisionId::new(key(20))],
        after.decisions[&DecisionId::new(fresh_key(20))]
    );
    assert_eq!(before.roots.decisions, after.roots.decisions);
}

#[test]
fn a_superseded_decision_names_its_successor_by_the_successors_decision_view_hash() {
    let mut document = small_document();
    let own = |n: u32| {
        if (20..30).contains(&n) {
            key(n + 40)
        } else {
            key(n)
        }
    };
    let mut successor = small_document_with(&own).decisions.remove(0);
    successor.title = "Keys are UUIDs, again".to_owned();
    document.decisions[0].state = DecisionState::Superseded {
        selected_option: OptionId::new(key(23)),
        outcome: "New keys are UUID version 7.".to_owned(),
        successor: successor.id,
    };
    let successor_id = successor.id;
    document.decisions.push(successor);
    let (hashes, views) = hash_indexed_with_views(&document.index().unwrap());
    let state = &views.decisions[&DecisionId::new(key(20))][2];
    assert_eq!(state["kind"], "superseded");
    assert_eq!(state["successor"], json!(hashes.decisions[&successor_id]));
}

#[test]
fn changing_an_outcome_changes_the_decisions_root_and_the_content_root() {
    let mut document = small_document();
    let before = roots(&document);
    document.decisions[0].state = DecisionState::Accepted {
        selected_option: OptionId::new(key(23)),
        outcome: "New keys are UUID version 7, minted by the formatter.".to_owned(),
    };
    let after = roots(&document);
    assert_ne!(after.decisions, before.decisions);
    assert_ne!(after.content, before.content);
}

#[test]
fn decision_order_leaves_the_decisions_root() {
    let mut document = small_document();
    let own = |n: u32| {
        if (20..30).contains(&n) {
            key(n + 40)
        } else {
            key(n)
        }
    };
    let mut second = small_document_with(&own).decisions.remove(0);
    second.title = "A second decision".to_owned();
    document.decisions.push(second);
    let before = roots(&document).decisions;
    document.decisions.reverse();
    assert_eq!(roots(&document).decisions, before);
}

#[test]
fn assessment_order_leaves_the_decision_view_hash() {
    let mut document = small_document();
    let id = DecisionId::new(key(20));
    let before = hash_document(&document).unwrap().decisions[&id];
    document.decisions[0].assessments.reverse();
    assert_eq!(hash_document(&document).unwrap().decisions[&id], before);
}

#[test]
fn a_bindings_content_hash_is_computed_from_the_tree() {
    let mut document = small_document();
    let before = roots(&document).bindings;
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some("Changed.".to_owned());
    assert_ne!(
        roots(&document).bindings,
        before,
        "F-1's content hash enters its binding"
    );
}

#[test]
fn every_state_view_field_moves_the_state_root() {
    let document = small_document();
    let before = roots(&document).state;
    let mut other_key = document.clone();
    other_key.id = DocumentId::new(key(99));
    assert_ne!(roots(&other_key).state, before, "document key");
    let mut derived = document.clone();
    derived.derived_from = Some(DerivedFrom {
        authored_state_id: AuthoredStateId::new(key(98)),
        content_address: "3823fe93df167a36ae3b73fb0d77a4630a5e42b40b639f6d45fd46909caca312"
            .parse()
            .unwrap(),
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
fn equal_state_views_have_the_same_state_root() {
    assert_eq!(
        roots(&small_document()).state,
        roots(&small_document()).state
    );
}

#[test]
fn an_edge_to_an_unknown_element_is_refused() {
    let mut document = small_document();
    document.edges.push(realizes(3, 77));
    assert_eq!(
        hash_document(&document).unwrap_err().integrity,
        [IntegrityError::UnknownEdgeEndpoint(
            ComponentId::new(key(77)).into()
        )]
    );
}

#[test]
fn an_edge_naming_a_key_as_the_wrong_kind_is_refused() {
    let mut document = small_document();
    document.edges.push(realizes(4, 6));
    assert_eq!(
        hash_document(&document).unwrap_err().integrity,
        [IntegrityError::UnknownEdgeEndpoint(
            FeatureId::new(key(4)).into()
        )]
    );
}

#[test]
fn a_binding_of_an_unknown_element_is_refused() {
    let mut document = small_document();
    document
        .bindings
        .bind(RequirementId::new(key(77)), label("RQ-77"))
        .unwrap();
    assert_eq!(
        hash_document(&document).unwrap_err().integrity,
        [IntegrityError::UnknownBindingElement(ElementId::from(
            RequirementId::new(key(77))
        ))]
    );
}

#[test]
fn a_key_carried_twice_is_refused() {
    let mut document = small_document();
    document.components.push(root(component(key(6), "Another")));
    assert_eq!(
        hash_document(&document).unwrap_err().integrity,
        [IntegrityError::DuplicateKey(key(6))]
    );
}

#[test]
fn the_tree_hashes_are_the_node_hashes() {
    let document = small_document();
    let hashes = hash_document(&document).unwrap();
    assert_eq!(hashes.products, [document.products[0].content_hash()]);
    let feature = &document.products[0].feature_groups[0].features[0];
    assert_eq!(
        hashes.nodes[&FeatureId::new(key(3)).into()],
        feature.content_hash()
    );
}

/// The small document's decision again, its element keys moved up by `by` and its title
/// changed, so its view hash differs.
fn another_decision(by: u32, title: &str) -> loftspec::decision::Decision {
    let own = |n: u32| {
        if (20..30).contains(&n) {
            key(n + by)
        } else {
            key(n)
        }
    };
    let mut decision = small_document_with(&own).decisions.remove(0);
    title.clone_into(&mut decision.title);
    decision
}

#[test]
fn the_decisions_root_is_over_the_decision_view_hashes_sorted_in_byte_order() {
    let mut document = small_document();
    document
        .decisions
        .insert(0, another_decision(60, "A third decision"));
    document
        .decisions
        .push(another_decision(40, "A second decision"));
    let (hashes, views) = hash_indexed_with_views(&document.index().unwrap());
    let in_key_order: Vec<Digest> = hashes.decisions.values().copied().collect();
    let mut sorted = in_key_order.clone();
    sorted.sort_by_key(|hash| *hash.as_bytes());
    assert_ne!(in_key_order, sorted, "key order is not byte order here");
    let (_, domain, view) = views
        .roots
        .iter()
        .find(|(name, _, _)| *name == "decisions")
        .unwrap();
    assert_eq!(*view, json!(sorted));
    assert_eq!(hashes.roots.decisions, digest(*domain, &sorted));
    assert_eq!(*domain, Domain::Decisions);
}

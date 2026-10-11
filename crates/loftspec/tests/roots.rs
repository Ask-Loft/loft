//! The edges, registry, decisions, content, binding and state roots (D-11 F-4, F-5).

mod common;

use common::{component, fresh_key, key, label, product, small_document, small_document_with};
use loftspec::canonical::Digest;
use loftspec::decision::DecisionState;
use loftspec::document::{CategoryEntry, DerivedFrom, Document, Edge};
use loftspec::hash::{ContentHash, HashError, Roots, hash_document};
use loftspec::ids::{
    AuthoredStateId, CategoryId, ComponentId, DecisionId, DocumentId, ElementId, FeatureId,
    OptionId, RequirementId,
};
use loftspec::names::CategoryName;

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
fn ac_49_and_ac_52_an_edge_enters_by_its_endpoints_content_so_rekeying_leaves_the_edges_root() {
    assert_eq!(
        roots(&small_document()).edges,
        roots(&small_document_with(&fresh_key)).edges
    );
}

#[test]
fn ac_54_the_same_document_with_fresh_keys_has_the_same_content_root() {
    assert_eq!(
        roots(&small_document()).content,
        roots(&small_document_with(&fresh_key)).content
    );
}

#[test]
fn ac_50_two_identical_edges_between_duplicate_nodes_both_count() {
    let mut document = small_document();
    document.components.push(component(key(40), "Twin"));
    document.components.push(component(key(41), "Twin"));
    document.edges.push(realizes(3, 40));
    let one = roots(&document).edges;
    document.edges.push(realizes(3, 41));
    assert_ne!(roots(&document).edges, one);
}

#[test]
fn ac_51_and_ac_252_adding_an_edge_changes_the_edges_root_and_the_content_root() {
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
fn ac_53_ac_55_and_ac_59_relabelling_moves_the_binding_root_and_nothing_else() {
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
fn ac_56_reordering_the_products_changes_the_content_root() {
    let mut document = small_document();
    document.products.push(product(key(50), "Second Product"));
    let before = roots(&document).content;
    document.products.reverse();
    assert_ne!(roots(&document).content, before);
}

#[test]
fn ac_57_and_ac_254_root_components_are_sorted_by_content_hash() {
    let mut document = small_document();
    document
        .components
        .push(component(key(51), "Second Component"));
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
    document.categories.push(CategoryEntry {
        id: CategoryId::new(key(33)),
        name: CategoryName::new("a.first"),
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
        .map(|tag| CategoryEntry {
            id: CategoryId::new(tag.id.key()),
            name: CategoryName::new(tag.name.as_str()),
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
fn ac_611_and_ac_612_fresh_element_keys_leave_the_decision_view_hash() {
    let before = hash_document(&small_document()).unwrap();
    let after = hash_document(&small_document_with(&fresh_key)).unwrap();
    assert_eq!(
        before.decisions[&DecisionId::new(key(20))],
        after.decisions[&DecisionId::new(fresh_key(20))]
    );
    assert_eq!(before.roots.decisions, after.roots.decisions);
}

#[test]
fn ac_221_and_ac_253_changing_an_outcome_changes_the_decisions_root_and_the_content_root() {
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
fn ac_222_decision_order_leaves_the_decisions_root() {
    let mut document = small_document();
    let mut second = document.decisions[0].clone();
    second.id = DecisionId::new(key(60));
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
fn ac_240_a_bindings_content_hash_is_computed_from_the_tree() {
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
fn ac_62_and_ac_63_every_state_view_field_moves_the_state_root() {
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
fn ac_64_equal_state_views_have_the_same_state_root() {
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
        hash_document(&document).unwrap_err(),
        HashError::UnknownEdgeEndpoint(ComponentId::new(key(77)).into())
    );
}

#[test]
fn an_edge_naming_a_key_as_the_wrong_kind_is_refused() {
    let mut document = small_document();
    document.edges.push(realizes(4, 6));
    assert_eq!(
        hash_document(&document).unwrap_err(),
        HashError::UnknownEdgeEndpoint(FeatureId::new(key(4)).into())
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
        hash_document(&document).unwrap_err(),
        HashError::UnknownBindingElement(ElementId::from(RequirementId::new(key(77))))
    );
}

#[test]
fn a_key_carried_twice_is_refused() {
    let mut document = small_document();
    document.components.push(component(key(6), "Another"));
    assert_eq!(
        hash_document(&document).unwrap_err(),
        HashError::DuplicateKey(key(6))
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

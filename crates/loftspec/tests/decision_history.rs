//! The decision history hash (Roots): the decision view with each node and successor it names
//! given by key, digested under `loftspec/decision-history/1`, so that editing what a decision
//! weighs leaves it unchanged and naming something else changes it. No root holds it.

use crate::common::{key, label, small_document, small_document_with};
use loftspec::canonical::{Digest, Domain, digest};
use loftspec::decision::{Decision, DecisionState, NodeRef};
use loftspec::document::Document;
use loftspec::hash::{
    decision_history_hash, decision_history_view, hash_document, hash_indexed_with_views,
};
use loftspec::ids::{ComponentId, DecisionId, OptionId};
use serde_json::{Value, json};

fn decision() -> DecisionId {
    DecisionId::new(key(20))
}

fn history(document: &Document) -> Digest {
    decision_history_hash(the_decision(document))
}

fn view(document: &Document) -> Value {
    decision_history_view(the_decision(document))
}

/// The decision of a document that indexes.
fn the_decision(document: &Document) -> &Decision {
    let indexed = document.index().unwrap();
    let decisions = &indexed.document().decisions;
    decisions.iter().find(|each| each.id == decision()).unwrap()
}

/// The small document with its decision superseded by a copy of it whose element keys are
/// moved up by 40, and that successor's key.
fn superseded() -> (Document, DecisionId) {
    let mut document = small_document();
    let own = |n: u32| {
        if (20..30).contains(&n) {
            key(n + 40)
        } else {
            key(n)
        }
    };
    let successor = small_document_with(&own).decisions.remove(0);
    document.decisions[0].state = DecisionState::Superseded {
        selected_option: OptionId::new(key(23)),
        outcome: "New keys are UUID version 7.".to_owned(),
        successor: successor.id,
    };
    let id = successor.id;
    document.decisions.push(successor);
    (document, id)
}

#[test]
fn the_history_view_is_the_decision_view_with_each_named_node_given_by_key() {
    let document = small_document();
    let (_, views) = hash_indexed_with_views(&document.index().unwrap());
    let mut expected = views.decisions[&decision()].clone();
    let requirement = json!(key(4).to_string());
    expected[3][1]["subject"] = requirement.clone();
    expected[6][0][2] = requirement;
    assert_ne!(expected, views.decisions[&decision()]);
    assert_eq!(view(&document), expected);
}

#[test]
fn the_history_view_names_a_successor_by_its_key() {
    let (document, successor) = superseded();
    assert_eq!(
        view(&document)[2]["successor"],
        json!(successor.key().to_string())
    );
}

#[test]
fn the_history_view_names_a_former_label_as_the_decision_view_does() {
    let mut document = small_document();
    document.decisions[0].consequences[0].subject = Some(NodeRef::Former(label("RQ-9")));
    let (_, views) = hash_indexed_with_views(&document.index().unwrap());
    let former = json!({ "formerLabel": "RQ-9" });
    assert_eq!(views.decisions[&decision()][6][0][2], former);
    assert_eq!(view(&document)[6][0][2], former);
}

#[test]
fn the_history_hash_is_the_digest_of_the_history_view_under_its_domain() {
    let document = small_document();
    let expected = digest(Domain::DecisionHistory, &view(&document));
    assert_eq!(history(&document), expected);
    assert_eq!(
        Domain::DecisionHistory.as_str(),
        "loftspec/decision-history/1"
    );
}

#[test]
fn editing_a_node_the_decision_names_leaves_its_history_hash() {
    let document = small_document();
    let mut edited = document.clone();
    let requirement = &mut edited.products[0].feature_groups[0].features[0].requirements[0];
    requirement.details.body = Some("Edited.".to_owned());
    let before = hash_document(&document).unwrap().decisions[&decision()];
    assert_ne!(
        hash_document(&edited).unwrap().decisions[&decision()],
        before
    );
    assert_eq!(history(&edited), history(&document));
}

#[test]
fn naming_another_node_changes_the_history_hash() {
    let document = small_document();
    let mut other = document.clone();
    other.decisions[0].consequences[0].subject = Some(ComponentId::new(key(6)).into());
    assert_ne!(history(&other), history(&document));
}

#[test]
fn editing_the_successor_leaves_the_history_hash() {
    let (document, successor) = superseded();
    let mut edited = document.clone();
    let at = edited
        .decisions
        .iter()
        .position(|d| d.id == successor)
        .unwrap();
    edited.decisions[at].title = "Keys are UUIDs, edited".to_owned();
    let before = hash_document(&document).unwrap().decisions[&decision()];
    assert_ne!(
        hash_document(&edited).unwrap().decisions[&decision()],
        before
    );
    assert_eq!(history(&edited), history(&document));
}

#[test]
fn the_history_hash_enters_no_root() {
    let document = small_document();
    let hashes = hash_document(&document).unwrap();
    let mut decision_hashes: Vec<Digest> = hashes.decisions.values().copied().collect();
    decision_hashes.sort_unstable();
    assert_eq!(
        hashes.roots.decisions,
        digest(Domain::Decisions, &decision_hashes)
    );
    let history = history(&document);
    assert!(!decision_hashes.contains(&history));
    let roots = hashes.roots;
    let all = [
        roots.edges,
        roots.tags,
        roots.categories,
        roots.decisions,
        roots.content,
    ];
    assert!(!all.contains(&history) && roots.bindings != history && roots.state != history);
}

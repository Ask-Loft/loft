//! What a publish records of each decision (Labels and aliases): the kind of its state, and for
//! a decision that is not proposed its decision history hash, in its binding; a departed
//! decision's binding loses both.

use crate::common::{key, label, proposed_decision, small_document};
use loftspec::canonical::Digest;
use loftspec::codec::{self, Files, LABELS, SpecPath};
use loftspec::decision::{DecisionState, StateKind};
use loftspec::document::Document;
use loftspec::hash::decision_history_hashes;
use loftspec::ids::DecisionId;
use loftspec::publish::{Publication, PublishedAt, ReadTree, RepoState, publish};

fn clean_main() -> RepoState {
    RepoState {
        branch: Some("main".to_owned()),
        modified: Vec::new(),
        stopped: None,
    }
}

/// The tree after a publish of `files`.
fn published(files: &Files) -> Files {
    let at = PublishedAt::from_unix_seconds(1_000).unwrap();
    let publication = publish(ReadTree::read(files).unwrap(), &clean_main(), None, at).unwrap();
    let Publication::Published { files: written, .. } = publication else {
        panic!("the tree is published");
    };
    let mut after = files.clone();
    after.extend(written);
    after
}

fn record(files: &Files, decision: DecisionId) -> (Option<StateKind>, Option<Digest>) {
    let document = codec::read(files).unwrap();
    let binding = document
        .bindings
        .iter()
        .find(|b| b.element == decision.into());
    let binding = binding.unwrap();
    (binding.published_state, binding.published_history)
}

fn history(document: &Document, decision: DecisionId) -> Digest {
    decision_history_hashes(&document.index().unwrap())[&decision]
}

/// The small document with a proposed decision beside its accepted one, and that decision's ID.
fn with_proposal() -> (Document, DecisionId) {
    let mut document = small_document();
    let proposal = proposed_decision(key(91), "a-proposal");
    let id = proposal.id;
    document.decisions.push(proposal);
    document.bindings.bind(id, label("ADR-2")).unwrap();
    (document, id)
}

#[test]
fn after_a_publish_each_decisions_binding_holds_the_kind_of_its_state() {
    let (document, proposal) = with_proposal();
    let after = published(&codec::write(&document));
    let accepted = DecisionId::new(key(20));
    assert_eq!(record(&after, accepted).0, Some(StateKind::Accepted));
    assert_eq!(record(&after, proposal).0, Some(StateKind::Proposed));
}

#[test]
fn a_decision_that_is_not_proposed_records_its_history_hash_and_a_proposed_one_none() {
    let (document, proposal) = with_proposal();
    let accepted = DecisionId::new(key(20));
    for state in [
        None,
        Some(DecisionState::Rejected {
            rationale: "Not now.".to_owned(),
        }),
    ] {
        let mut document = document.clone();
        if let Some(state) = state {
            document.decisions[0].state = state;
        }
        let after = published(&codec::write(&document));
        let read = codec::read(&after).unwrap();
        assert_eq!(record(&after, accepted).1, Some(history(&read, accepted)));
        assert_eq!(record(&after, proposal), (Some(StateKind::Proposed), None));
    }
}

#[test]
fn a_published_history_is_written_in_lowercase_hexadecimal() {
    let after = published(&codec::write(&small_document()));
    let labels = String::from_utf8(after[&SpecPath::new(LABELS)].clone()).unwrap();
    let line = labels
        .lines()
        .find_map(|line| line.strip_prefix("    publishedHistory: "))
        .unwrap();
    assert_eq!(line.len(), 64);
    assert!(
        line.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert!(labels.contains("    publishedState: accepted\n    publishedHistory: "));
}

#[test]
fn a_publish_removes_the_record_of_a_departed_decision() {
    let mut document = small_document();
    let gone = DecisionId::new(key(90));
    let history = "ab".repeat(32).parse::<Digest>().unwrap();
    document.bindings.bind(gone, label("ADR-5")).unwrap();
    let bindings = &mut document.bindings;
    bindings
        .record_published(gone, Some(StateKind::Accepted), Some(history))
        .unwrap();
    bindings.depart(gone).unwrap();
    let after = published(&codec::write(&document));
    assert_eq!(record(&after, gone), (None, None));
}

#[test]
fn the_record_of_a_decision_is_what_the_last_publish_that_held_it_wrote() {
    let accepted = DecisionId::new(key(20));
    let first = published(&codec::write(&small_document()));
    let mut document = codec::read(&first).unwrap();
    document.decisions[0].state = DecisionState::Rejected {
        rationale: "Not now.".to_owned(),
    };
    let edited = codec::write(&document);
    assert_eq!(record(&edited, accepted).0, Some(StateKind::Accepted));
    let second = published(&edited);
    assert_eq!(record(&second, accepted).0, Some(StateKind::Rejected));
}

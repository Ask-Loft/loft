//! Citations of withdrawn labels (F-59): a publish rewrites a
//! label withdrawn from a key to that key's live label, in the text of every element whose
//! `mentions` include the key, and only there.

use crate::common::text::{FEATURE, files, path, text};
use crate::common::{key, label};
use loftspec::codec::{self, Files};
use loftspec::hash::hash_document;
use loftspec::publish::rewrite::rewrite;
use loftspec::publish::{Publication, PublishedAt, ReadTree, Relabel, RepoState, publish};

const DECISION: &str = "decisions/keys-are-uuids.yaml";
const CRITERION_NAME: &str = "name: \"Every node record carries a `key` field.\"";
const MENTIONS_4: &str = "        mentions:\n          - 00000000-0000-7000-8000-000000000004\n";

/// The small document's files with each `(file, from, to)` replaced once.
fn tree(edits: &[(&str, &str, &str)]) -> Files {
    let mut files = files();
    for (at, from, to) in edits {
        let current = text(&files, at);
        assert!(current.contains(from), "{from:?} in {at}");
        files.insert(path(at), current.replacen(from, to, 1).into_bytes());
    }
    files
}

/// `RQ-9` withdrawn from the Requirement, whose live label is `RQ-2`.
const WITHDRAWN: (&str, &str, &str) = (
    "labels.yaml",
    "      - RQ-1\n",
    "      - RQ-1\n    withdrawn:\n      - RQ-9\n",
);

/// The criterion, which mentions the Requirement, citing `citation`.
fn citing(citation: &'static str) -> (&'static str, &'static str, String) {
    (
        FEATURE,
        CRITERION_NAME,
        format!("name: \"Every node record carries a `key` field, as {citation} says.\""),
    )
}

fn rewritten_tree(edits: &[(&str, &str, &str)]) -> (Files, loftspec::publish::rewrite::Rewrites) {
    let files = tree(edits);
    let document = codec::read(&files).unwrap();
    let rewrites = rewrite(&files, &document.bindings);
    (files, rewrites)
}

#[test]
fn a_citation_in_an_element_that_mentions_the_key_is_rewritten_to_its_live_label() {
    let (from, to) = (CRITERION_NAME, citing("RQ-9").2);
    let (_, rewrites) = rewritten_tree(&[WITHDRAWN, (FEATURE, from, &to)]);
    assert_eq!(
        rewrites.rewritten,
        [Relabel {
            element: key(5),
            withdrawn: label("RQ-9"),
            label: label("RQ-2"),
        }]
    );
    assert_eq!(rewrites.files.keys().collect::<Vec<_>>(), [&path(FEATURE)]);
    let feature = String::from_utf8(rewrites.files[&path(FEATURE)].clone()).unwrap();
    assert!(
        feature.contains("carries a `key` field, as RQ-2 says.\""),
        "{feature}"
    );
    assert_eq!(
        feature,
        text(&tree(&[(FEATURE, from, &citing("RQ-2").2)]), FEATURE)
    );
}

#[test]
fn a_citation_in_an_element_that_does_not_mention_the_key_is_left() {
    let body = (
        FEATURE,
        "body: \"Keys address nodes.\"",
        "body: \"Keys address nodes; see RQ-9.\"",
    );
    let (_, rewrites) = rewritten_tree(&[WITHDRAWN, body]);
    assert_eq!(rewrites.rewritten, Vec::new());
    assert_eq!(rewrites.files, Files::new());
}

#[test]
fn a_citation_qualified_by_a_document_is_left() {
    let (from, to) = (CRITERION_NAME, citing("D-7/RQ-9").2);
    let (_, rewrites) = rewritten_tree(&[WITHDRAWN, (FEATURE, from, &to)]);
    assert_eq!(rewrites.rewritten, Vec::new());
}

#[test]
fn a_citation_in_an_element_that_also_mentions_the_label_s_holder_is_left() {
    let holder = (
        "labels.yaml",
        "    label: F-1\n",
        "    label: F-1\n    aliases:\n      - RQ-9\n",
    );
    let both = (
        FEATURE,
        MENTIONS_4,
        "        mentions:\n          - 00000000-0000-7000-8000-000000000003\n          - 00000000-0000-7000-8000-000000000004\n",
    );
    let (from, to) = (CRITERION_NAME, citing("RQ-9").2);
    let (_, rewrites) = rewritten_tree(&[WITHDRAWN, holder, both, (FEATURE, from, &to)]);
    assert_eq!(rewrites.rewritten, Vec::new());
}

#[test]
fn a_label_withdrawn_from_two_keys_the_element_mentions_is_left() {
    let second = (
        "labels.yaml",
        "    label: F-1\n",
        "    label: F-1\n    withdrawn:\n      - RQ-9\n",
    );
    let both = (
        FEATURE,
        MENTIONS_4,
        "        mentions:\n          - 00000000-0000-7000-8000-000000000003\n          - 00000000-0000-7000-8000-000000000004\n",
    );
    let (from, to) = (CRITERION_NAME, citing("RQ-9").2);
    let (_, rewrites) = rewritten_tree(&[WITHDRAWN, second, both, (FEATURE, from, &to)]);
    assert_eq!(rewrites.rewritten, Vec::new());
}

#[test]
fn a_decision_s_option_text_is_rewritten_by_the_decision_s_mentions() {
    let option = (
        DECISION,
        "description: \"Time-ordered RFC 9562 UUIDs.\"",
        "description: \"Time-ordered RFC 9562 UUIDs, as RQ-9 asks.\"",
    );
    let mentions = (
        DECISION,
        "    node: 00000000-0000-7000-8000-000000000004\n",
        "    node: 00000000-0000-7000-8000-000000000004\nmentions:\n  - 00000000-0000-7000-8000-000000000004\n",
    );
    let (_, rewrites) = rewritten_tree(&[WITHDRAWN, option, mentions]);
    assert_eq!(
        rewrites.rewritten,
        [Relabel {
            element: key(20),
            withdrawn: label("RQ-9"),
            label: label("RQ-2"),
        }]
    );
    let decision = String::from_utf8(rewrites.files[&path(DECISION)].clone()).unwrap();
    assert!(decision.contains("UUIDs, as RQ-2 asks.\""), "{decision}");
}

#[test]
fn a_decision_s_driver_and_consequence_text_is_rewritten_by_the_decision_s_mentions() {
    let driver = (
        DECISION,
        "statement: \"Converted and new keys share one syntax.\"",
        "statement: \"Converted and new keys share one syntax, as RQ-9 asks.\"",
    );
    let consequence = (
        DECISION,
        "statement: \"A key reveals when it was minted.\"",
        "statement: \"A key reveals when it was minted, despite RQ-9.\"",
    );
    let mentions = (
        DECISION,
        "    node: 00000000-0000-7000-8000-000000000004\n",
        "    node: 00000000-0000-7000-8000-000000000004\nmentions:\n  - 00000000-0000-7000-8000-000000000004\n",
    );
    let (_, rewrites) = rewritten_tree(&[WITHDRAWN, driver, consequence, mentions]);
    let citation = Relabel {
        element: key(20),
        withdrawn: label("RQ-9"),
        label: label("RQ-2"),
    };
    assert_eq!(rewrites.rewritten, [citation.clone(), citation]);
    let decision = String::from_utf8(rewrites.files[&path(DECISION)].clone()).unwrap();
    assert!(
        decision.contains("one syntax, as RQ-2 asks.\""),
        "{decision}"
    );
    assert!(decision.contains("minted, despite RQ-2.\""), "{decision}");
    assert!(!decision.contains("RQ-9"), "{decision}");
}

#[test]
fn a_tree_without_a_withdrawn_label_is_not_rewritten() {
    let (from, to) = (CRITERION_NAME, citing("RQ-9").2);
    let (_, rewrites) = rewritten_tree(&[(FEATURE, from, &to)]);
    assert_eq!(rewrites.rewritten, Vec::new());
    assert_eq!(rewrites.files, Files::new());
}

#[test]
fn a_publish_writes_the_rewritten_files_and_pins_the_tree_it_writes() {
    let (from, to) = (CRITERION_NAME, citing("RQ-9").2);
    let files = tree(&[WITHDRAWN, (FEATURE, from, &to)]);
    let repo = RepoState {
        branch: Some("main".to_owned()),
        modified: Vec::new(),
        stopped: None,
    };
    let at = PublishedAt::from_unix_seconds(0).unwrap();
    let published = publish(ReadTree::read(&files).unwrap(), &repo, None, at).unwrap();
    let Publication::Published {
        lock,
        files: written,
        rewritten,
        ..
    } = published
    else {
        panic!("a publish");
    };
    assert_eq!(rewritten.len(), 1);
    let mut next = files.clone();
    next.extend(written.clone());
    assert!(text(&next, FEATURE).contains("as RQ-2 says."));
    let roots = hash_document(&codec::read(&next).unwrap()).unwrap().roots;
    assert_eq!(lock.state_root, roots.state);
    assert!(
        written
            .keys()
            .all(|written| *written == path(FEATURE) || *written == path("labels.yaml"))
    );
}

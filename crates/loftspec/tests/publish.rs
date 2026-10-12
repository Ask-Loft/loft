//! The publisher (F-27 to F-31): its preconditions, the lock and bindings it gives, idempotence,
//! and verification of a lock against the tree.

mod common;

use std::num::NonZeroU64;

use common::{key, small_document};
use loftspec::canonical::Digest;
use loftspec::codec::{self, Files, LABELS, SpecPath};
use loftspec::decision::NodeRef;
use loftspec::document::{DerivedFrom, Document, Edge, SubjectRole};
use loftspec::hash::hash_document;
use loftspec::ids::{AuthoredStateId, DecisionId};
use loftspec::publish::{
    Lock, Publication, PublishedAt, ReadTree, Refusal, RepoState, Root, Unverified, publish, verify,
};

/// Publishes a tree that reads.
fn publish_files(
    files: &Files,
    repo: &RepoState,
    previous: Option<&Lock>,
    at: PublishedAt,
) -> Result<Publication, Refusal> {
    publish(ReadTree::read(files).unwrap(), repo, previous, at)
}

/// Verifies a tree that reads.
fn verify_files(files: &Files, lock: &Lock) -> Result<Digest, Unverified> {
    verify(&ReadTree::read(files).unwrap(), lock)
}

fn clean_on(branch: &str) -> RepoState {
    RepoState {
        branch: Some(branch.to_owned()),
        modified: Vec::new(),
    }
}

fn at(seconds: u64) -> PublishedAt {
    PublishedAt::from_unix_seconds(seconds).unwrap()
}

fn derived() -> Document {
    let mut document = small_document();
    document.derived_from = Some(DerivedFrom {
        authored_state_id: AuthoredStateId::new(key(90)),
        content_address: "f".repeat(64).parse().unwrap(),
        publish_sequence: 3,
    });
    document
}

/// Publishes `files` on a clean `main` with no previous lock.
fn first(files: &Files) -> (Lock, Vec<u8>) {
    match publish_files(files, &clean_on("main"), None, at(1_000)).unwrap() {
        Publication::Published { lock, labels, .. } => (lock, labels.expect("new bindings")),
        Publication::Unchanged(_) => panic!("a first publish publishes"),
    }
}

/// The tree with the bindings a publish wrote.
fn with_labels(files: &Files, labels: Vec<u8>) -> Files {
    let mut files = files.clone();
    files.insert(SpecPath::new(LABELS), labels);
    files
}

#[test]
fn ac_447_ac_448_uncommitted_changes_under_spec_refuse_naming_the_paths() {
    let repo = RepoState {
        modified: vec!["spec/tags.yaml".to_owned()],
        ..clean_on("main")
    };
    let refusal = publish_files(&codec::write(&small_document()), &repo, None, at(0)).unwrap_err();
    assert_eq!(
        refusal,
        Refusal::Modified(vec!["spec/tags.yaml".to_owned()])
    );
}

#[test]
fn ac_449_ac_450_a_branch_other_than_main_refuses_naming_both_branches() {
    let refusal = publish_files(
        &codec::write(&small_document()),
        &clean_on("develop"),
        None,
        at(0),
    )
    .unwrap_err();
    assert_eq!(
        refusal,
        Refusal::Branch {
            current: Some("develop".to_owned()),
            publish: "main".to_owned(),
        }
    );
}

#[test]
fn ac_449_the_manifest_can_name_another_publish_branch() {
    let mut document = small_document();
    document.publish_branch = Some("release".to_owned());
    let files = codec::write(&document);
    assert!(publish_files(&files, &clean_on("release"), None, at(0)).is_ok());
    assert!(matches!(
        publish_files(&files, &clean_on("main"), None, at(0)),
        Err(Refusal::Branch { .. })
    ));
}

#[test]
fn ac_690_a_detached_head_is_refused_as_not_on_the_publish_branch() {
    let repo = RepoState::default();
    let refusal = publish_files(&codec::write(&small_document()), &repo, None, at(0)).unwrap_err();
    assert_eq!(
        refusal,
        Refusal::Branch {
            current: None,
            publish: "main".to_owned(),
        }
    );
}

#[test]
fn ac_451_ac_452_a_validation_error_refuses_listing_the_errors() {
    // An accepted decision may not name a former label (RQ-179).
    let mut document = small_document();
    document.edges.push(Edge::Subject {
        decision: DecisionId::new(key(20)),
        subject: NodeRef::Former(common::label("AC-9")),
        role: SubjectRole::Affected,
    });
    let files = codec::write(&document);
    assert!(codec::read(&files).is_ok(), "the tree reads");
    let Err(Refusal::Invalid(errors)) = publish_files(&files, &clean_on("main"), None, at(0))
    else {
        panic!("a validation error refuses the publish");
    };
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains("AC-9")),
        "{errors:?}"
    );
}

#[test]
fn ac_453_a_lint_finding_does_not_refuse_a_publish() {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0].requirements[0].criteria[0].name =
        "The writer is fast and/or cheap, etc.".to_owned();
    let files = codec::write(&document);
    assert!(
        !loftspec::check::check(&files).findings().is_empty(),
        "the tree has a finding"
    );
    assert!(publish_files(&files, &clean_on("main"), None, at(0)).is_ok());
}

#[test]
fn a_tree_that_does_not_read_is_never_published() {
    let mut files = codec::write(&small_document());
    files.insert(SpecPath::new("stray.yaml"), b"x: 1\n".to_vec());
    assert!(ReadTree::read(&files).is_err());
}

#[test]
fn bindings_whose_bytes_would_not_change_are_not_given() {
    let files = codec::write(&small_document());
    let (lock, labels) = first(&files);
    let mut document = codec::read(&with_labels(&files, labels)).unwrap();
    document.products[0].name = "Keys and labels".to_owned();
    let changed = publish_files(
        &codec::write(&document),
        &clean_on("main"),
        Some(&lock),
        at(0),
    );
    let Ok(Publication::Published { labels, .. }) = changed else {
        panic!("a changed tree publishes");
    };
    assert_eq!(labels, None);
}

#[test]
fn ac_454_ac_460_a_publish_labels_every_node_and_changes_only_the_bindings() {
    let files = codec::write(&small_document());
    let (_, labels) = first(&files);
    let published = codec::read(&with_labels(&files, labels.clone())).unwrap();
    published.walk(&mut |node| {
        assert!(published.bindings.label_of(node.id().unwrap()).is_some());
    });
    let mut expected = small_document();
    loftspec::publish::allocate(&mut expected);
    assert_eq!(labels, codec::write(&expected)[&SpecPath::new(LABELS)]);
}

#[test]
fn ac_72_ac_73_the_lock_pins_the_hashers_roots_of_the_published_tree() {
    let files = codec::write(&small_document());
    let (lock, labels) = first(&files);
    let published = codec::read(&with_labels(&files, labels)).unwrap();
    let roots = hash_document(&published).unwrap().roots;
    assert_eq!(
        (lock.content_root, lock.binding_root, lock.state_root),
        (roots.content, roots.bindings, roots.state)
    );
    assert_eq!(lock.document_key, published.id);
}

#[test]
fn ac_461_ac_462_a_first_publish_is_sequence_1_at_the_given_time() {
    let (lock, _) = first(&codec::write(&small_document()));
    assert_eq!(lock.publish_sequence, NonZeroU64::MIN);
    assert_eq!(lock.published_at, at(1_000));
}

#[test]
fn ac_74_ac_75_ac_255_ac_77_the_lock_carries_the_source_state_of_a_derived_document() {
    let document = derived();
    let (lock, _) = first(&codec::write(&document));
    assert_eq!(lock.derived_from, document.derived_from);
    // AC-77: carried unchanged into the next lock.
    let mut changed = document.clone();
    changed.products[0].name = "Keys and labels".to_owned();
    let next = publish_files(
        &codec::write(&changed),
        &clean_on("main"),
        Some(&lock),
        at(2_000),
    );
    let Ok(Publication::Published { lock: next, .. }) = next else {
        panic!("a changed tree publishes");
    };
    assert_eq!(next.derived_from, document.derived_from);
}

#[test]
fn ac_76_the_lock_of_a_document_that_is_not_derived_has_no_source_state() {
    let (lock, _) = first(&codec::write(&small_document()));
    assert_eq!(lock.derived_from, None);
}

#[test]
fn ac_464_ac_465_ac_466_ac_640_a_second_publish_over_an_unchanged_tree_is_a_no_op() {
    let files = codec::write(&small_document());
    let (lock, labels) = first(&files);
    let published = with_labels(&files, labels);
    let again = publish_files(&published, &clean_on("main"), Some(&lock), at(2_000)).unwrap();
    assert_eq!(again, Publication::Unchanged(lock));
}

#[test]
fn ac_461_a_publish_after_a_change_is_one_more_than_the_previous_sequence() {
    let files = codec::write(&small_document());
    let (lock, labels) = first(&files);
    let mut document = codec::read(&with_labels(&files, labels)).unwrap();
    document.products[0].name = "Keys and labels".to_owned();
    let next = publish_files(
        &codec::write(&document),
        &clean_on("main"),
        Some(&lock),
        at(2_000),
    );
    let Ok(Publication::Published {
        lock: next,
        allocated,
        ..
    }) = next
    else {
        panic!("a changed tree publishes");
    };
    assert_eq!(next.publish_sequence.get(), 2);
    assert_eq!(allocated, []);
}

#[test]
fn ac_641_an_uncommitted_change_refuses_even_when_nothing_changed_since_the_last_publish() {
    let files = codec::write(&small_document());
    let (lock, labels) = first(&files);
    let repo = RepoState {
        modified: vec!["spec/labels.yaml".to_owned()],
        ..clean_on("main")
    };
    assert!(matches!(
        publish_files(&with_labels(&files, labels), &repo, Some(&lock), at(0)),
        Err(Refusal::Modified(_))
    ));
}

/// A published tree and its lock.
fn published() -> (Files, Lock) {
    let files = codec::write(&derived());
    let (lock, labels) = first(&files);
    (with_labels(&files, labels), lock)
}

fn mismatch(files: &Files, lock: &Lock) -> Root {
    match verify_files(files, lock) {
        Err(Unverified::Mismatch { root, .. }) => root,
        other => panic!("a mismatch, not {other:?}"),
    }
}

#[test]
fn ac_468_verification_passes_when_the_state_root_matches() {
    let (files, lock) = published();
    assert_eq!(verify_files(&files, &lock), Ok(lock.state_root));
}

#[test]
fn ac_467_ac_469_a_changed_content_root_fails_naming_it_first() {
    let (files, lock) = published();
    let mut document = codec::read(&files).unwrap();
    document.products[0].name = "Changed".to_owned();
    document.bindings = codec::read(&files).unwrap().bindings;
    assert_eq!(mismatch(&codec::write(&document), &lock), Root::Content);
}

#[test]
fn ac_678_a_relabel_alone_fails_on_the_binding_root() {
    let (files, lock) = published();
    let mut document = codec::read(&files).unwrap();
    document
        .bindings
        .relabel(loftspec::ids::ProductId::new(key(1)), common::label("P-9"))
        .unwrap();
    assert_eq!(mismatch(&codec::write(&document), &lock), Root::Binding);
}

#[test]
fn ac_678_a_changed_source_state_alone_fails_on_the_state_root() {
    let (files, lock) = published();
    let mut document = codec::read(&files).unwrap();
    if let Some(derived) = &mut document.derived_from {
        derived.publish_sequence = 4;
    }
    assert_eq!(mismatch(&codec::write(&document), &lock), Root::State);
}

#[test]
fn ac_468_a_lock_of_other_roots_never_verifies() {
    let (files, mut lock) = published();
    lock.state_root = "0".repeat(64).parse::<Digest>().unwrap();
    assert_eq!(mismatch(&files, &lock), Root::State);
}

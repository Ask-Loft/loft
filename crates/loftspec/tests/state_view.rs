//! State views (Roots): state view 1 is the state view as it stood before the lint configuration
//! entered it; state view 2 ends with the digest of `spec/lint.yaml` where the tree holds it.

use std::collections::BTreeMap;

use crate::common::small_document;
use loftspec::canonical::{Domain, digest};
use loftspec::codec::{self, Files, LINT as LINT_FILE, Problem, SpecPath, lint_configuration};
use loftspec::document::Document;
use loftspec::ext::ExtValue;
use loftspec::hash::{StateView, hash_document, lint_digest, state_root, state_view};
use loftspec::names::Namespace;
use loftspec::publish::{
    Lock, Made, Publication, PublishedAt, ReadTree, RepoState, Root, Unverified, publish, verify,
};
use loftspec::yaml::{ErrorKind, Location};
use serde_json::{Value, json};

const LINT: &str = "rules:\n  - rule: LQ-005\n    enabled: false\n";
/// Another lint configuration.
const OTHER: &str = "rules:\n  - rule: LQ-005\n    enabled: true\n";

fn root_of(
    document: &Document,
    view: StateView,
    lint: Option<&str>,
) -> loftspec::canonical::Digest {
    let roots = hash_document(document).unwrap().roots;
    state_root(document, &roots, view, lint)
}

fn extended() -> Document {
    let mut document = small_document();
    let namespace = Namespace::parse("openspec").unwrap();
    let value = ExtValue::Text("kept".to_owned());
    document.products[0].details.ext = BTreeMap::from([(namespace, value)]);
    document
}

#[test]
fn a_state_view_of_a_tree_with_a_lint_configuration_ends_with_its_digest() {
    for document in [small_document(), extended()] {
        let roots = hash_document(&document).unwrap().roots;
        let one = state_view(&document, &roots, StateView::One, Some(LINT));
        let two = state_view(&document, &roots, StateView::Two, Some(LINT));
        let (Value::Array(one), Value::Array(two)) = (one, two) else {
            panic!("a state view is an array");
        };
        assert_eq!(two.len(), one.len() + 1);
        assert_eq!(two[..one.len()], one[..]);
        let last = json!({ "lintConfiguration": lint_digest(LINT) });
        assert_eq!(two.last(), Some(&last));
        if let Some(extensions) = roots.extensions {
            assert_eq!(two[two.len() - 2], json!(extensions));
        }
    }
    assert!(
        hash_document(&extended())
            .unwrap()
            .roots
            .extensions
            .is_some()
    );
}

#[test]
fn the_lint_digest_is_over_the_files_whole_text_as_one_json_string() {
    assert_eq!(lint_digest(LINT), digest(Domain::Lint, &json!(LINT)));
    assert_eq!(Domain::Lint.as_str(), "loftspec/lint/1");
}

#[test]
fn a_tree_without_a_lint_configuration_has_one_state_root_under_both_views() {
    let document = small_document();
    let one = root_of(&document, StateView::One, None);
    assert_eq!(root_of(&document, StateView::Two, None), one);
    assert_eq!(one, hash_document(&document).unwrap().roots.state);
}

#[test]
fn under_state_view_2_any_change_of_the_lint_configuration_changes_the_state_root() {
    let document = small_document();
    let with = root_of(&document, StateView::Two, Some(LINT));
    let without = root_of(&document, StateView::Two, None);
    assert_ne!(with, without, "adding or removing the file");
    let relaid = format!("{} \n", LINT.trim_end());
    let changed = root_of(&document, StateView::Two, Some(&relaid));
    assert_ne!(changed, with, "a trailing space");
}

#[test]
fn the_lint_configuration_leaves_the_content_and_binding_roots_unchanged() {
    let document = small_document();
    let roots = hash_document(&document).unwrap().roots;
    let with = state_root(&document, &roots, StateView::Two, Some(LINT));
    assert_ne!(with, roots.state);
    assert_eq!(hash_document(&document).unwrap().roots, roots);
}

#[test]
fn under_state_view_1_the_lint_configuration_leaves_the_state_root_unchanged() {
    let document = small_document();
    let state = hash_document(&document).unwrap().roots.state;
    for lint in [None, Some(LINT), Some(OTHER)] {
        assert_eq!(root_of(&document, StateView::One, lint), state);
    }
}

#[test]
fn the_state_views_are_numbered_one_and_two() {
    assert_eq!(StateView::One.number(), 1);
    assert_eq!(StateView::Two.number(), 2);
    assert_eq!(StateView::HIGHEST, StateView::Two);
    for (number, view) in [
        (1, Some(StateView::One)),
        (2, Some(StateView::Two)),
        (3, None),
    ] {
        assert_eq!(StateView::from_number(number), view);
    }
}

fn clean_main() -> RepoState {
    RepoState {
        branch: Some("main".to_owned()),
        modified: Vec::new(),
        stopped: None,
    }
}

/// `files` with `spec/lint.yaml` holding `lint`, or without the file.
fn linted(files: &Files, lint: Option<&str>) -> Files {
    let mut files = files.clone();
    match lint {
        Some(lint) => files.insert(SpecPath::new(LINT_FILE), lint.as_bytes().to_vec()),
        None => files.remove(&SpecPath::new(LINT_FILE)),
    };
    files
}

/// Publishes `files` after `previous`: the publication, and the tree with what it wrote.
fn publish_after(files: &Files, previous: Option<&Lock>) -> (Publication, Files) {
    let at = PublishedAt::from_unix_seconds(1_000).unwrap();
    let publication = publish(
        ReadTree::read(files).unwrap(),
        Some(&clean_main()),
        previous,
        at,
        Made::NotAsRemote,
    )
    .unwrap();
    let mut tree = files.clone();
    if let Publication::Published { files: written, .. } = &publication {
        tree.extend(written.clone());
    }
    (publication, tree)
}

fn published(publication: Publication) -> Lock {
    match publication {
        Publication::Published { lock, .. } => lock,
        Publication::Unchanged(_) => panic!("the tree is published"),
    }
}

fn verifies(files: &Files, lock: &Lock) -> bool {
    verify(&ReadTree::read(files).unwrap(), lock).is_ok()
}

/// A lock of state view 1 for `files` and the tree as that publish left it.
fn view_one_lock(files: &Files) -> (Lock, Files) {
    let (publication, tree) = publish_after(&linted(files, None), None);
    let lock = published(publication);
    assert_eq!(lock.state_view, StateView::One);
    let ours = linted(&tree, lint_configuration(files).unwrap());
    (lock, ours)
}

#[test]
fn the_lock_of_a_tree_with_a_lint_configuration_names_state_view_2_and_its_view_2_root() {
    let (publication, tree) =
        publish_after(&linted(&codec::write(&small_document()), Some(LINT)), None);
    let lock = published(publication);
    assert_eq!(lock.state_view, StateView::Two);
    assert!(
        String::from_utf8(lock.to_bytes())
            .unwrap()
            .contains("\"stateView\": 2,")
    );
    let document = codec::read(&tree).unwrap();
    assert_eq!(
        lock.state_root,
        root_of(&document, StateView::Two, Some(LINT))
    );
    assert_ne!(
        lock.state_root,
        hash_document(&document).unwrap().roots.state
    );
}

#[test]
fn the_lock_of_a_tree_without_a_lint_configuration_names_no_state_view() {
    let (publication, _) = publish_after(&codec::write(&small_document()), None);
    let bytes = published(publication).to_bytes();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("stateView"), "{text}");
    let fields: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("  \"")?.split('"').next())
        .collect();
    let view_one = [
        "format",
        "algorithm",
        "documentKey",
        "stateRoot",
        "contentRoot",
        "bindingRoot",
        "publishSequence",
        "publishedAt",
    ];
    assert_eq!(fields, view_one);
}

#[test]
fn a_tree_with_a_lint_configuration_published_under_state_view_1_is_published_once_more() {
    let (lock, tree) = view_one_lock(&linted(&codec::write(&small_document()), Some(LINT)));
    let (publication, after) = publish_after(&tree, Some(&lock));
    let next = published(publication);
    assert_eq!(next.publish_sequence.get(), lock.publish_sequence.get() + 1);
    assert_eq!(next.state_view, StateView::Two);
    assert_eq!(next.content_root, lock.content_root);
    assert_eq!(next.binding_root, lock.binding_root);
    let (again, _) = publish_after(&after, Some(&next));
    assert_eq!(again, Publication::Unchanged(next));
}

#[test]
fn a_tree_without_a_lint_configuration_published_under_state_view_1_is_not_published_again() {
    let (lock, tree) = view_one_lock(&codec::write(&small_document()));
    let (publication, _) = publish_after(&tree, Some(&lock));
    assert_eq!(publication, Publication::Unchanged(lock));
}

#[test]
fn a_lock_without_a_state_view_is_verified_under_state_view_1_whatever_its_lint_configuration() {
    let (lock, tree) = view_one_lock(&codec::write(&small_document()));
    let bytes = String::from_utf8(lock.to_bytes()).unwrap();
    assert!(!bytes.contains("stateView"));
    let read = Lock::read(bytes.as_bytes()).unwrap();
    for lint in [None, Some(LINT), Some(OTHER), None] {
        assert!(verifies(&linted(&tree, lint), &read), "{lint:?}");
    }
}

#[test]
fn a_lock_of_state_view_2_is_verified_under_state_view_2() {
    let files = linted(&codec::write(&small_document()), Some(LINT));
    let (publication, tree) = publish_after(&files, None);
    let lock = published(publication);
    assert!(verifies(&tree, &lock));
    let changed = verify(&ReadTree::read(&linted(&tree, Some(OTHER))).unwrap(), &lock);
    assert!(
        matches!(
            changed,
            Err(Unverified::Mismatch {
                root: Root::State,
                ..
            })
        ),
        "{changed:?}"
    );
    assert!(!verifies(&linted(&tree, None), &lock));
}

#[test]
fn a_lint_configuration_that_is_not_utf8_is_refused_naming_the_file_and_the_byte_offset() {
    let mut files = codec::write(&small_document());
    let invalid = b"rules:\n  - rule: \xff\n".to_vec();
    files.insert(SpecPath::new(LINT_FILE), invalid);
    let refused = |errors: Vec<codec::ReadError>| {
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].sites[0].path, SpecPath::new(LINT_FILE));
        assert_eq!(errors[0].sites[0].location, Some(Location::Byte(17)));
        assert_eq!(errors[0].problem, Problem::Syntax(ErrorKind::InvalidUtf8));
    };
    refused(ReadTree::read(&files).err().unwrap());
    refused(vec![lint_configuration(&files).unwrap_err()]);
    let good = linted(&files, Some(LINT));
    assert_eq!(lint_configuration(&good).unwrap(), Some(LINT));
    assert_eq!(lint_configuration(&linted(&files, None)).unwrap(), None);
}

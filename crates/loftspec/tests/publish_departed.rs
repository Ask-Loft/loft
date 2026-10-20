//! A publish over departed bindings (Label allocation at publish): a key that comes back takes
//! back the label it departed with, or keeps it as an alias where it came back as another kind,
//! and the binding of a key that left is marked departed; no binding is removed.

use crate::common::decisions::decision_id;
use crate::common::text::{COMPONENT, DECISION, FEATURE, files, path, text};
use crate::common::{key, label, small_document};
use loftspec::check::check;
use loftspec::codec::{self, Files, LABELS};
use loftspec::fmt::format;
use loftspec::identity::LabelPrefix;
use loftspec::ids::{ConstraintId, RequirementId};
use loftspec::publish::{
    Allocated, Made, NamedAsFormer, Publication, PublishedAt, ReadTree, Refusal, RepoState,
    publish, publish_labels,
};

const CONSTRAINT: &str = "00000000-0000-7000-8000-000000000007";

fn formatted(files: Files) -> Files {
    let result = format(&files).unwrap();
    let mut files = files;
    files.extend(result.changed);
    files
}

/// The tree with its Component's Constraint `CON-1` removed.
fn without_constraint(files: &Files) -> Files {
    let component = text(files, COMPONENT);
    let start = component.find("constraints:\n").unwrap();
    let end = component.find("qualityAttributes:\n").unwrap();
    let mut files = files.clone();
    let edited = format!("{}{}", &component[..start], &component[end..]);
    files.insert(path(COMPONENT), edited.into_bytes());
    files
}

fn clean_main() -> RepoState {
    RepoState {
        branch: Some("main".to_owned()),
        modified: Vec::new(),
        stopped: None,
    }
}

fn attempt(files: &Files) -> Result<Publication, Refusal> {
    let at = PublishedAt::from_unix_seconds(1_000).unwrap();
    publish(
        ReadTree::read(files).unwrap(),
        Some(&clean_main()),
        None,
        at,
        Made::NotAsRemote,
    )
}

fn publication_of(files: &Files) -> Publication {
    attempt(files).unwrap()
}

/// The tree after a first publish.
fn published(files: &Files) -> Files {
    let Publication::Published { files: written, .. } = publication_of(files) else {
        panic!("the tree is published");
    };
    let mut after = files.clone();
    after.extend(written);
    after
}

#[test]
fn a_node_deleted_and_restored_between_publishes_takes_back_its_label() {
    // Delete the node, format, restore it, format again, then publish.
    let gone = formatted(without_constraint(&files()));
    assert!(text(&gone, LABELS).contains("    label: CON-1\n    departed: true\n"));
    let mut back = gone.clone();
    back.insert(path(COMPONENT), files()[&path(COMPONENT)].clone());
    let back = formatted(back);
    let Publication::Published {
        given_back,
        allocated,
        ..
    } = publication_of(&back)
    else {
        panic!("the tree is published");
    };
    let constraint = ConstraintId::new(key(7)).into();
    let returned = Allocated {
        label: label("CON-1"),
        element: constraint,
    };
    assert_eq!(given_back, [returned]);
    assert!(allocated.iter().all(|given| given.element != constraint));
    let after = published(&back);
    let document = codec::read(&after).unwrap();
    assert_eq!(document.bindings.label_of(constraint), Some(label("CON-1")));
    let binding = document.bindings.iter().find(|b| b.element == constraint);
    assert!(!binding.unwrap().departed);
    // The counters are those of a publish of the tree the node never left.
    let never_left = published(&files());
    assert_eq!(
        document.counters,
        codec::read(&never_left).unwrap().counters
    );
    assert!(text(&after, LABELS).contains(&format!("  - key: {CONSTRAINT}\n    label: CON-1\n")));
}

#[test]
fn a_key_that_comes_back_as_another_kind_keeps_its_departed_label_as_an_alias() {
    let mut document = small_document();
    let constraint = ConstraintId::new(key(7));
    document.bindings.unbind(constraint);
    let before = RequirementId::new(key(7));
    document.bindings.bind(before, label("RQ-9")).unwrap();
    document.bindings.depart(before).unwrap();
    let counters = document.counters.clone();
    let labels = publish_labels(&mut document);
    assert_eq!(labels.given_back, []);
    let next = label("CON-3");
    assert!(labels.allocated.contains(&Allocated {
        label: next,
        element: constraint.into(),
    }));
    assert_eq!(document.bindings.label_of(constraint), Some(next));
    assert_eq!(document.bindings.aliases_of(constraint), [label("RQ-9")]);
    assert_eq!(document.counters.next(LabelPrefix::Constraint).get(), 4);
    assert!(counters.next(LabelPrefix::Constraint).get() <= 3);
}

#[test]
fn the_binding_of_a_key_that_left_is_marked_departed_and_none_is_removed() {
    let mut document = small_document();
    document.components[0].component.constraints.clear();
    let count = document.bindings.len();
    let labels = publish_labels(&mut document);
    let constraint = ConstraintId::new(key(7)).into();
    let left = Allocated {
        label: label("CON-1"),
        element: constraint,
    };
    assert_eq!(labels.departed, [left]);
    assert_eq!(document.bindings.len(), count + labels.allocated.len());
    let binding = document.bindings.iter().find(|b| b.element == constraint);
    assert!(binding.unwrap().departed);
    assert_eq!(document.bindings.label_of(constraint), None);
    document.index().unwrap();
}

#[test]
fn a_binding_already_departed_is_left_as_it_is_by_a_publish() {
    let mut document = small_document();
    document.components[0].component.constraints.clear();
    publish_labels(&mut document);
    let before = document.bindings.clone();
    let again = publish_labels(&mut document);
    assert_eq!(again.departed, []);
    assert_eq!(document.bindings, before);
}

#[test]
fn a_publish_after_a_lock_that_gives_a_label_back_is_a_publish() {
    let first = published(&files());
    let at = PublishedAt::from_unix_seconds(2_000).unwrap();
    let Publication::Published { lock, .. } = publication_of(&files()) else {
        panic!("the tree is published");
    };
    let gone = formatted(without_constraint(&first));
    let mut back = gone.clone();
    back.insert(path(COMPONENT), first[&path(COMPONENT)].clone());
    let back = formatted(back);
    let repo = clean_main();
    let again = publish(
        ReadTree::read(&back).unwrap(),
        Some(&repo),
        Some(&lock),
        at,
        Made::NotAsRemote,
    );
    let Ok(Publication::Published {
        given_back,
        lock: next,
        ..
    }) = again
    else {
        panic!("a label given back is published: {again:?}");
    };
    assert_eq!(given_back.len(), 1);
    assert_eq!(next.publish_sequence.get(), 2);
}

/// The tree with the Requirement `RQ-2` and its criterion deleted from their Feature, and the
/// decision that names it retired, as `loft fmt` leaves it: the decision names `RQ-2` under
/// `formerLabel`.
fn without_requirement_formatted() -> Files {
    let feature = text(&files(), FEATURE);
    let start = feature.find("requirements:\n").unwrap();
    let end = feature.find("qualityAttributes:\n").unwrap();
    let mut files = files();
    let edited = format!("{}{}", &feature[..start], &feature[end..]);
    files.insert(path(FEATURE), edited.into_bytes());
    let decision = text(&files, DECISION).replacen(
        "  kind: accepted\n",
        "  kind: retired\n  rationale: \"Gone.\"\n",
        1,
    );
    files.insert(path(DECISION), decision.into_bytes());
    let files = formatted(files);
    assert!(text(&files, DECISION).contains("formerLabel: RQ-2\n"));
    files
}

/// The tree published without `RQ-2`, then its Feature restored as it was and not formatted.
fn restored_unformatted() -> Files {
    let mut back = published(&without_requirement_formatted());
    back.insert(path(FEATURE), files()[&path(FEATURE)].clone());
    back
}

#[test]
fn a_publish_that_would_give_back_a_label_a_decision_names_as_former_is_refused() {
    let refused = attempt(&restored_unformatted());
    let Err(Refusal::NamedAsFormer(named)) = refused else {
        panic!("the publish is refused: {refused:?}");
    };
    let expected = NamedAsFormer {
        label: label("RQ-2"),
        decisions: vec![decision_id()],
    };
    assert_eq!(named, [expected]);
    let message = named[0].to_string();
    assert!(message.contains("RQ-2"), "{message}");
    assert!(
        message.contains(&decision_id().key().to_string()),
        "{message}"
    );
    assert!(message.contains("formatting writes"), "{message}");
    assert!(message.contains("as the node's key"), "{message}");
}

#[test]
fn the_tree_formatted_after_that_refusal_is_published_and_checks_clean() {
    let back = formatted(restored_unformatted());
    assert!(!text(&back, DECISION).contains("formerLabel"));
    let after = published(&back);
    let checked = check(&after);
    assert!(!checked.has_errors(), "{checked:?}");
    let document = codec::read(&after).unwrap();
    let requirement = RequirementId::new(key(4));
    assert_eq!(document.bindings.label_of(requirement), Some(label("RQ-2")));
}

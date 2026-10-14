//! Settling two publishes (F-58): a publish run in a stopped merge or
//! rebase of the publish branch keeps the pulled side's bindings, withdraws the other side's
//! colliding labels, relabels a key that lost its live label, and pins the tree it writes.

use std::fmt::Write as _;
use std::num::NonZeroU64;

use crate::common::text::{files, path};
use crate::common::{key, label};
use loftspec::codec::{self, Files, SpecPath};
use loftspec::document::Document;
use loftspec::hash::hash_document;
use loftspec::identity::LabelPrefix;
use loftspec::ids::ElementId;
use loftspec::merge::merge_file;
use loftspec::publish::{
    Lock, Operation, Publication, PublishedAt, ReadTree, Refusal, Relabel, RepoState, Stopped,
    publish,
};

/// A binding as a labels file lists it: the key's number, its live label and its aliases.
type Bind = (u32, &'static str, &'static [&'static str]);

/// The labels a key withdrew: the key's number and the labels.
type Withdrawn = (u32, &'static [&'static str]);

/// The small document's bindings but those of the criterion and the two constraints.
const KEPT: [Bind; 8] = [
    (0x1, "P-1", &[]),
    (0x2, "FG-1", &[]),
    (0x3, "F-1", &[]),
    (0x4, "RQ-2", &["RQ-1"]),
    (0x6, "CMP-1", &[]),
    (0x8, "QA-1", &[]),
    (0xa, "QA-2", &[]),
    (0x14, "ADR-1", &[]),
];

/// A labels file holding the kept bindings and `extra`, sorted by key.
fn labels(extra: &[Bind]) -> String {
    withdrawing(extra, &[])
}

/// A labels file holding the kept bindings and `extra`, sorted by key, each key `withdrawn`
/// names listing the labels given with it as withdrawn.
fn withdrawing(extra: &[Bind], withdrawn: &[Withdrawn]) -> String {
    let mut all: Vec<Bind> = KEPT.iter().chain(extra).copied().collect();
    all.sort_by_key(|bind| bind.0);
    let mut text = String::from(
        "counters:\n  P: 1\n  FG: 1\n  F: 1\n  RQ: 1\n  AC: 1\n  CMP: 1\n  CON: 1\n  QA: 1\n  ADR: 1\nbindings:\n",
    );
    for (n, live, aliases) in all {
        let _ = write!(text, "  - key: {}\n    label: {live}\n", key(n));
        if !aliases.is_empty() {
            text.push_str("    aliases:\n");
            for alias in aliases {
                let _ = writeln!(text, "      - {alias}");
            }
        }
        for (_, labels) in withdrawn.iter().filter(|listed| listed.0 == n) {
            text.push_str("    withdrawn:\n");
            for label in *labels {
                let _ = writeln!(text, "      - {label}");
            }
        }
    }
    text
}

/// The small document's tree, its bindings the merge of `local` and `pulled` from a start that
/// binds neither constraint nor the criterion, as `operation` hands git's merge driver the sides.
fn met(operation: Operation, local: &[Bind], pulled: &[Bind]) -> Files {
    met_files(operation, labels(local), labels(pulled))
}

/// The small document's tree, its bindings the merge of the labels files `local` and `pulled`
/// from a start that binds neither constraint nor the criterion, as [`met`] merges them.
fn met_files(operation: Operation, local: String, pulled: String) -> Files {
    let base = labels(&[]);
    let (ours, theirs) = match operation {
        Operation::Merge => (local, pulled),
        Operation::Rebase => (pulled, local),
    };
    let at = SpecPath::new("labels.yaml");
    let merged = merge_file(&at, base.as_bytes(), ours.as_bytes(), theirs.as_bytes()).unwrap();
    assert!(merged.conflicts > 0, "the sides dispute a binding");
    let mut files = files();
    files.insert(path("labels.yaml"), merged.bytes);
    files
}

/// A lock of the small document with publish sequence `sequence`.
fn lock(sequence: u64) -> Vec<u8> {
    let repo = RepoState {
        branch: Some("main".to_owned()),
        ..RepoState::default()
    };
    let at = PublishedAt::from_unix_seconds(0).unwrap();
    let Ok(Publication::Published { lock, .. }) =
        publish(ReadTree::read(&files()).unwrap(), &repo, None, at)
    else {
        panic!("the small document publishes");
    };
    Lock {
        publish_sequence: NonZeroU64::new(sequence).unwrap(),
        ..lock
    }
    .to_bytes()
}

/// A repository stopped in `operation` of `main`, the locks at sequences 4 and 6, with the
/// bindings and the lock unmerged and the bindings changed.
fn stopped(operation: Operation) -> RepoState {
    RepoState {
        branch: (operation == Operation::Merge).then(|| "main".to_owned()),
        modified: vec!["spec/labels.yaml".to_owned()],
        stopped: Some(Stopped {
            operation,
            branch: Some("main".to_owned()),
            head_lock: Some(lock(4)),
            other_lock: Some(lock(6)),
            unmerged: vec!["loft.lock.json".to_owned(), "spec/labels.yaml".to_owned()],
        }),
    }
}

fn settle(files: &Files, repo: &RepoState) -> Result<Publication, Refusal> {
    let at = PublishedAt::from_unix_seconds(100).unwrap();
    publish(ReadTree::read(files).unwrap(), repo, None, at)
}

/// What a publish gave, and the tree with the files it wrote.
struct Settled {
    lock: Lock,
    withdrawn: Vec<Relabel>,
    finish: Option<Operation>,
    tree: Files,
    document: Document,
}

fn settled(files: &Files, repo: &RepoState) -> Settled {
    let Ok(Publication::Published {
        lock,
        files: written,
        withdrawn,
        finish,
        ..
    }) = settle(files, repo)
    else {
        panic!("the publish settles");
    };
    let mut tree = files.clone();
    tree.extend(written);
    let document = codec::read(&tree).unwrap();
    Settled {
        lock,
        withdrawn,
        finish,
        tree,
        document,
    }
}

/// The local side binds the second constraint `CON-2`; the pulled side gives it the first.
const LOCAL: [Bind; 1] = [(0x9, "CON-2", &[])];
const PULLED: [Bind; 1] = [(0x7, "CON-2", &[])];

fn assert_pulled_side_kept(settled: &Settled) {
    let bindings = &settled.document.bindings;
    assert_eq!(
        bindings.resolve(label("CON-2")).map(ElementId::key),
        Some(key(7))
    );
    assert_eq!(
        bindings.withdrawn_of(bindings.resolve(label("CON-3")).unwrap()),
        [label("CON-2")]
    );
    assert_eq!(
        bindings.resolve(label("CON-3")).map(ElementId::key),
        Some(key(9))
    );
    assert_eq!(
        bindings.aliases_of(bindings.resolve(label("CON-3")).unwrap()),
        []
    );
    assert_eq!(settled.document.conflicts, Vec::new());
    assert_eq!(
        settled.withdrawn,
        [Relabel {
            element: key(9),
            withdrawn: label("CON-2"),
            label: label("CON-3"),
        }]
    );
}

#[test]
fn in_a_stopped_merge_the_pulled_side_is_theirs_and_the_other_key_is_relabelled() {
    let files = met(Operation::Merge, &LOCAL, &PULLED);
    let settled = settled(&files, &stopped(Operation::Merge));
    assert_pulled_side_kept(&settled);
    assert_eq!(settled.finish, Some(Operation::Merge));
}

#[test]
fn in_a_stopped_rebase_the_pulled_side_is_ours() {
    let files = met(Operation::Rebase, &LOCAL, &PULLED);
    let settled = settled(&files, &stopped(Operation::Rebase));
    assert_pulled_side_kept(&settled);
    assert_eq!(settled.finish, Some(Operation::Rebase));
}

#[test]
fn the_settled_tree_checks_and_the_lock_pins_it_one_past_the_higher_sequence() {
    let files = met(Operation::Merge, &LOCAL, &PULLED);
    let settled = settled(&files, &stopped(Operation::Merge));
    assert_eq!(settled.lock.publish_sequence.get(), 7);
    let roots = hash_document(&settled.document).unwrap().roots;
    assert_eq!(settled.lock.state_root, roots.state);
    assert_eq!(loftspec::validate::validate(&settled.document), Vec::new());
}

#[test]
fn a_key_both_sides_labelled_apart_keeps_the_pulled_label_and_withdraws_the_other() {
    let files = met(
        Operation::Merge,
        &[(0x5, "AC-2", &[])],
        &[(0x5, "AC-3", &[])],
    );
    let settled = settled(&files, &stopped(Operation::Merge));
    let bindings = &settled.document.bindings;
    let criterion = bindings.resolve(label("AC-3")).unwrap();
    assert_eq!(criterion.key(), key(5));
    assert_eq!(bindings.withdrawn_of(criterion), [label("AC-2")]);
    assert_eq!(bindings.aliases_of(criterion), []);
}

#[test]
fn a_withdrawn_list_only_grows_when_a_key_that_lists_one_withdraws_another() {
    let earlier: [Withdrawn; 1] = [(0x5, &["AC-1"])];
    let files = met_files(
        Operation::Merge,
        withdrawing(&[(0x5, "AC-2", &[])], &earlier),
        withdrawing(&[(0x5, "AC-3", &[])], &earlier),
    );
    let settled = settled(&files, &stopped(Operation::Merge));
    let bindings = &settled.document.bindings;
    let criterion = bindings.resolve(label("AC-3")).unwrap();
    assert_eq!(criterion.key(), key(5));
    assert_eq!(
        bindings.withdrawn_of(criterion),
        [label("AC-1"), label("AC-2")]
    );
}

#[test]
fn the_counter_is_raised_above_a_withdrawn_label_of_a_key_that_lost_its_live_label() {
    let earlier: [Withdrawn; 1] = [(0x9, &["CON-12"])];
    let files = met_files(
        Operation::Merge,
        withdrawing(&LOCAL, &earlier),
        labels(&PULLED),
    );
    let settled = settled(&files, &stopped(Operation::Merge));
    let document = &settled.document;
    let live = document.bindings.label_of_key(key(9)).unwrap();
    let constraint = document.bindings.resolve(live).unwrap();
    assert!(
        document
            .bindings
            .withdrawn_of(constraint)
            .contains(&label("CON-12"))
    );
    assert!(document.counters.next(LabelPrefix::Constraint).get() > 12);
}

#[test]
fn a_key_that_lost_only_an_alias_keeps_its_live_label_and_its_other_aliases() {
    let local: [Bind; 1] = [(0x9, "CON-5", &["CON-2", "CON-4"])];
    let files = met(Operation::Merge, &local, &PULLED);
    let settled = settled(&files, &stopped(Operation::Merge));
    let bindings = &settled.document.bindings;
    let constraint = bindings.resolve(label("CON-5")).unwrap();
    assert_eq!(constraint.key(), key(9));
    assert_eq!(bindings.aliases_of(constraint), [label("CON-4")]);
    assert_eq!(bindings.withdrawn_of(constraint), [label("CON-2")]);
    assert_eq!(
        bindings.resolve(label("CON-2")).map(ElementId::key),
        Some(key(7))
    );
}

#[test]
fn the_sequence_is_1_when_neither_commit_holds_a_lock() {
    let files = met(Operation::Merge, &LOCAL, &PULLED);
    let mut repo = stopped(Operation::Merge);
    let stopped = repo.stopped.as_mut().unwrap();
    (stopped.head_lock, stopped.other_lock) = (None, None);
    assert_eq!(settled(&files, &repo).lock.publish_sequence.get(), 1);
}

#[test]
fn a_second_publish_of_the_settled_tree_changes_nothing() {
    let files = met(Operation::Merge, &LOCAL, &PULLED);
    let repo = stopped(Operation::Merge);
    let first = settled(&files, &repo);
    let at = PublishedAt::from_unix_seconds(200).unwrap();
    let again = publish(
        ReadTree::read(&first.tree).unwrap(),
        &repo,
        Some(&first.lock),
        at,
    );
    assert_eq!(again, Ok(Publication::Unchanged(first.lock)));
}

#[test]
fn an_unmerged_path_other_than_the_bindings_and_the_lock_is_refused_by_name() {
    let files = met(Operation::Merge, &LOCAL, &PULLED);
    let mut repo = stopped(Operation::Merge);
    let unmerged = &mut repo.stopped.as_mut().unwrap().unmerged;
    unmerged.extend([":/src/main.rs".to_owned(), "spec/tags.yaml".to_owned()]);
    assert_eq!(
        settle(&files, &repo),
        Err(Refusal::Unmerged(vec![
            ":/src/main.rs".to_owned(),
            "spec/tags.yaml".to_owned()
        ]))
    );
}

#[test]
fn a_stopped_merge_or_rebase_of_another_branch_or_a_detached_head_is_refused() {
    let files = met(Operation::Merge, &LOCAL, &PULLED);
    for (operation, branch) in [
        (Operation::Merge, Some("feature")),
        (Operation::Rebase, Some("feature")),
        (Operation::Rebase, None),
    ] {
        let mut repo = stopped(operation);
        repo.stopped.as_mut().unwrap().branch = branch.map(str::to_owned);
        assert_eq!(
            settle(&files, &repo),
            Err(Refusal::Branch {
                current: branch.map(str::to_owned),
                publish: "main".to_owned(),
            }),
            "{operation:?} of {branch:?}"
        );
    }
}

#[test]
fn a_lock_at_a_commit_that_is_not_a_lock_record_is_refused() {
    let files = met(Operation::Merge, &LOCAL, &PULLED);
    let mut repo = stopped(Operation::Merge);
    repo.stopped.as_mut().unwrap().other_lock = Some(b"<<<<<<< ours\n".to_vec());
    assert!(matches!(settle(&files, &repo), Err(Refusal::Lock { .. })));
}

#[test]
fn unmerged_bindings_without_a_conflict_record_are_refused() {
    let mut repo = stopped(Operation::Merge);
    repo.branch = Some("main".to_owned());
    assert_eq!(settle(&files(), &repo), Err(Refusal::UnmergedBindings));
}

#[test]
fn bindings_merged_without_a_conflict_publish_when_only_the_lock_is_unmerged() {
    let mut repo = stopped(Operation::Merge);
    repo.stopped.as_mut().unwrap().unmerged = vec!["loft.lock.json".to_owned()];
    assert!(matches!(
        settle(&files(), &repo),
        Ok(Publication::Published { .. })
    ));
}

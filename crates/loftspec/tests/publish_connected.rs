//! The publisher given no repository state, and the publish of a connected document (Repository
//! preconditions, Connected publication): only the preconditions that always apply refuse a
//! publish given no repository state; a connected document's publish not made as its remote is
//! refused where it would give a label, and is made on any branch; one made as its remote
//! allocates as any publish does; and each publish is based on the state its lock names.

use std::num::NonZeroU64;

use crate::common::{key, label, small_document};
use loftspec::canonical::Digest;
use loftspec::codec::{self, Files};
use loftspec::decision::NodeRef;
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::ids::{ConstraintId, DecisionId};
use loftspec::publish::{
    Allocated, Base, Lock, Made, Operation, Publication, PublishedAt, ReadTree, Refusal, RepoState,
    Stopped, publish,
};

fn at() -> PublishedAt {
    PublishedAt::from_unix_seconds(1_000).unwrap()
}

fn on(branch: Option<&str>) -> RepoState {
    RepoState {
        branch: branch.map(str::to_owned),
        modified: Vec::new(),
        stopped: None,
    }
}

fn run(
    files: &Files,
    repo: Option<&RepoState>,
    previous: Option<&Lock>,
    made: Made,
) -> Result<Publication, Refusal> {
    publish(ReadTree::read(files).unwrap(), repo, previous, at(), made)
}

/// The tree with what a publish wrote, and its lock.
fn applied(files: &Files, publication: Publication) -> (Files, Lock) {
    let Publication::Published {
        lock,
        files: written,
        ..
    } = publication
    else {
        panic!("the tree is published");
    };
    let mut files = files.clone();
    files.extend(written);
    (files, lock)
}

/// The small document marked connected, every node labelled by a publish made as its remote,
/// and that publish's lock.
fn connected() -> (Files, Lock) {
    let mut document = small_document();
    document.connected = true;
    let files = codec::write(&document);
    let made = run(&files, None, None, Made::AsRemote).unwrap();
    applied(&files, made)
}

fn allocated(publication: &Publication) -> Vec<Allocated> {
    match publication {
        Publication::Published { allocated, .. } => allocated.clone(),
        Publication::Unchanged(_) => Vec::new(),
    }
}

/// The connected tree with a change that needs no label: the Product's body.
fn changed(files: &Files) -> Files {
    let mut document = codec::read(files).unwrap();
    document.products[0].details.body = Some("Changed.".to_owned());
    codec::write(&document)
}

#[test]
fn a_publish_given_no_repository_state_is_refused_on_no_branch_or_change() {
    let mut document = small_document();
    document.publish_branch = Some("release".to_owned());
    let publication = run(&codec::write(&document), None, None, Made::NotAsRemote);
    assert!(
        matches!(publication, Ok(Publication::Published { .. })),
        "{publication:?}"
    );
}

#[test]
fn a_validation_error_refuses_a_publish_given_no_repository_state() {
    let mut document = small_document();
    document.edges.push(Edge::Subject {
        decision: DecisionId::new(key(20)),
        subject: NodeRef::Former(label("AC-9")),
        role: SubjectRole::Affected,
    });
    let refused = run(&codec::write(&document), None, None, Made::NotAsRemote);
    assert!(matches!(refused, Err(Refusal::Invalid(_))), "{refused:?}");
}

#[test]
fn a_publish_given_no_repository_state_allocates_as_any_other_publish() {
    let files = codec::write(&small_document());
    let without = run(&files, None, None, Made::NotAsRemote).unwrap();
    let with = run(&files, Some(&on(Some("main"))), None, Made::NotAsRemote).unwrap();
    assert_eq!(without, with);
    assert_ne!(allocated(&without), []);
}

#[test]
fn a_connected_documents_publish_not_made_as_its_remote_is_made_on_any_branch() {
    let (files, lock) = connected();
    let files = changed(&files);
    for branch in [Some("feature"), None] {
        let publication = run(&files, Some(&on(branch)), Some(&lock), Made::NotAsRemote);
        assert!(
            matches!(publication, Ok(Publication::Published { .. })),
            "{branch:?}"
        );
        assert_eq!(allocated(&publication.unwrap()), []);
    }
    let mut document = codec::read(&files).unwrap();
    document.publish_branch = Some("release".to_owned());
    let release = codec::write(&document);
    let publication = run(
        &release,
        Some(&on(Some("main"))),
        Some(&lock),
        Made::NotAsRemote,
    );
    assert!(
        matches!(publication, Ok(Publication::Published { .. })),
        "{publication:?}"
    );
}

#[test]
fn a_connected_documents_publish_not_made_as_its_remote_that_would_give_a_label_is_refused() {
    let (files, lock) = connected();
    let mut document = codec::read(&files).unwrap();
    document.bindings.unbind(ConstraintId::new(key(7)));
    let unlabelled = codec::write(&document);
    for repo in [None, Some(on(Some("main")))] {
        let refused = run(&unlabelled, repo.as_ref(), Some(&lock), Made::NotAsRemote);
        assert_eq!(refused, Err(Refusal::Connected), "{repo:?}");
    }
    let as_remote = run(&unlabelled, None, Some(&lock), Made::AsRemote).unwrap();
    let given = Allocated {
        label: label("CON-3"),
        element: ConstraintId::new(key(7)).into(),
    };
    assert_eq!(allocated(&as_remote), [given]);
}

#[test]
fn a_connected_documents_publish_not_made_as_its_remote_in_a_stopped_merge_is_refused() {
    let (files, lock) = connected();
    for operation in [Operation::Merge, Operation::Rebase] {
        let repo = RepoState {
            stopped: Some(Stopped {
                operation,
                branch: Some("main".to_owned()),
                head_lock: Some(lock.to_bytes()),
                other_lock: Some(lock.to_bytes()),
                unmerged: Vec::new(),
            }),
            ..on(Some("main"))
        };
        let refused = run(&files, Some(&repo), Some(&lock), Made::NotAsRemote);
        assert_eq!(refused, Err(Refusal::Connected), "{operation:?}");
    }
}

#[test]
fn a_publish_made_as_the_remote_allocates_as_a_publish_of_a_document_not_connected() {
    let plain = codec::write(&small_document());
    let mut document = small_document();
    document.connected = true;
    let linked = codec::write(&document);
    let reference = allocated(&run(&plain, None, None, Made::NotAsRemote).unwrap());
    assert_eq!(
        allocated(&run(&plain, None, None, Made::AsRemote).unwrap()),
        reference
    );
    let as_remote = run(&linked, Some(&on(Some("feature"))), None, Made::AsRemote);
    assert!(
        matches!(as_remote, Err(Refusal::Branch { .. })),
        "{as_remote:?}"
    );
    assert_eq!(
        allocated(&run(&linked, None, None, Made::AsRemote).unwrap()),
        reference
    );
}

#[test]
fn a_publish_made_as_the_remote_gives_back_a_departed_label() {
    let (files, lock) = connected();
    let mut document = codec::read(&files).unwrap();
    let constraint = ConstraintId::new(key(7));
    document.bindings.depart(constraint).unwrap();
    let departed = codec::write(&document);
    let refused = run(&departed, None, Some(&lock), Made::NotAsRemote);
    assert_eq!(refused, Err(Refusal::Connected));
    let Publication::Published { given_back, .. } =
        run(&departed, None, Some(&lock), Made::AsRemote).unwrap()
    else {
        panic!("the tree is published");
    };
    let back = Allocated {
        label: label("CON-1"),
        element: constraint.into(),
    };
    assert_eq!(given_back, [back]);
}

#[test]
fn a_publish_is_based_on_the_publish_sequence_and_state_root_of_its_lock() {
    let (files, lock) = connected();
    let files = changed(&files);
    let behind = Lock {
        publish_sequence: NonZeroU64::new(5).unwrap(),
        state_root: "ab".repeat(32).parse::<Digest>().unwrap(),
        ..lock
    };
    let publication = run(&files, None, Some(&behind), Made::NotAsRemote).unwrap();
    let Publication::Published { base, lock, .. } = publication else {
        panic!("a lock behind the head refuses no publish");
    };
    assert_eq!(
        base,
        Base {
            publish_sequence: 5,
            state_root: Some(behind.state_root),
        }
    );
    assert_eq!(lock.publish_sequence.get(), 6);
}

#[test]
fn a_connected_document_without_a_lock_is_based_on_sequence_0_and_no_state_root() {
    let mut document: Document = small_document();
    document.connected = true;
    let files = codec::write(&document);
    let Publication::Published { base, .. } = run(&files, None, None, Made::AsRemote).unwrap()
    else {
        panic!("the tree is published");
    };
    let none = Base {
        publish_sequence: 0,
        state_root: None,
    };
    assert_eq!(base, none);
    assert_eq!(Base::of(None), none);
}

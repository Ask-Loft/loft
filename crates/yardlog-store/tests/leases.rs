//! The lease a running wait holds on its submission: a lock in git's own directory for the
//! worktree, held by one wait at a time, free once the process that holds it ends however it
//! ends, and nothing of it under `.yard/`.

use crate::common::{Tree, holding};
use yardlog::SubmissionKey;

#[test]
fn a_lease_is_a_lock_in_git_s_own_directory_for_the_worktree() {
    let tree = Tree::new();
    let place = tree.place(&tree.on("main"), &tree.top()).unwrap();
    let key = SubmissionKey::mint();
    let lease = place.lease(key).unwrap().expect("the lease is free");
    assert!(lease.path().starts_with(tree.git_dir().join("yard")));
    assert!(!tree.top().join(".yard").exists());
}

#[test]
fn one_wait_at_a_time_holds_a_submission_s_lease() {
    let tree = Tree::new();
    let place = tree.place(&tree.on("main"), &tree.top()).unwrap();
    let key = SubmissionKey::mint();
    let first = place.lease(key).unwrap().expect("the lease is free");
    assert!(place.lease(key).unwrap().is_none());
    assert!(place.is_leased(key).unwrap());
    assert!(place.lease(SubmissionKey::mint()).unwrap().is_some());
    drop(first);
    assert!(!place.is_leased(key).unwrap());
    assert!(place.lease(key).unwrap().is_some());
}

#[test]
fn a_lease_is_free_once_the_process_that_holds_it_is_killed() {
    let tree = Tree::new();
    let place = tree.place(&tree.on("main"), &tree.top()).unwrap();
    let key = SubmissionKey::mint();
    let mut child = holding(&place.lease_path(key), &tree.scratch.0.join("ready"));
    assert!(place.is_leased(key).unwrap());
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!place.is_leased(key).unwrap());
    assert!(place.lease(key).unwrap().is_some());
}

#[test]
fn a_lease_s_holder_is_named_while_it_holds_it_and_its_name_goes_with_it() {
    let tree = Tree::new();
    let place = tree.place(&tree.on("main"), &tree.top()).unwrap();
    let key = SubmissionKey::mint();
    let named = place.lease_path(key).with_extension("holder");
    let first = place
        .lease_as(key, b"first")
        .unwrap()
        .expect("the lease is free");
    assert_eq!(std::fs::read(&named).unwrap(), b"first");
    drop(first);
    assert!(
        !named.exists(),
        "the name of a holder that let go stays behind"
    );
    let second = place
        .lease_as(key, b"second")
        .unwrap()
        .expect("the lease is free");
    assert_eq!(std::fs::read(&named).unwrap(), b"second");
    drop(second);
}

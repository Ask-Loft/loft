//! Which Yard Log a call reaches: the one of the branch that the worktree holding the call's
//! directory is on, under `.yard/` at the top of that worktree, opened by the first write on the
//! branch with a key of its own; never one on a detached HEAD or outside a repository.

use std::fs;

use crate::common::{Scratch, Tree, read, write};
use serde_json::json;
use software_plant::GitScenario;
use yardlog_store::{Place, YardStore, YardStoreError};

fn stage(id: &str) -> serde_json::Value {
    json!([{"id": id, "kind": "stage", "title": id}])
}

#[test]
fn the_first_write_on_a_branch_opens_its_yard_log_with_a_key_its_branch_and_its_title() {
    let tree = Tree::new();
    let store = tree.store("feat/x");
    let written = write(&store, &stage("design"));
    assert_eq!(written.value.revision, 1);
    assert!(tree.top().join(".yard/feat/x/yardlog.yaml").is_file());
    let log = read(&store);
    assert_eq!(log.root.branch, "feat/x");
    assert_eq!(log.root.title, "feat/x");
    assert_eq!(log.root.opened.revision, 1);
    assert_eq!(log.root.key.key().version(), 7);
}

#[test]
fn a_later_write_on_the_same_branch_reaches_the_same_yard_log() {
    let tree = Tree::new();
    write(&tree.store("main"), &stage("design"));
    let key = read(&tree.store("main")).root.key;
    write(&tree.store("main"), &stage("build"));
    let log = read(&tree.store("main"));
    assert_eq!(log.root.key, key);
    assert_eq!(log.root.revision, 2);
}

#[test]
fn a_write_on_another_branch_opens_that_branch_s_own_yard_log() {
    let tree = Tree::new();
    write(&tree.store("main"), &stage("design"));
    let main = tree.scratch.snapshot();
    write(&tree.store("other"), &stage("build"));
    assert_ne!(
        read(&tree.store("other")).root.key,
        read(&tree.store("main")).root.key
    );
    let after = tree.scratch.snapshot();
    let main_files = |files: &std::collections::BTreeMap<String, Vec<u8>>| {
        files
            .iter()
            .filter(|(path, _)| path.starts_with("tree/.yard/main"))
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(main_files(&after), main_files(&main));
    assert!(tree.top().join(".yard/other/yardlog.yaml").is_file());
}

#[test]
fn a_call_below_the_top_of_the_worktree_reaches_the_yard_log_at_the_top() {
    let tree = Tree::new();
    let sub = tree.top().join("sub/deep");
    fs::create_dir_all(&sub).unwrap();
    let place = tree
        .place(&tree.on("main").below("sub/deep/"), &sub)
        .unwrap();
    assert_eq!(place.tree, tree.top());
    write(&YardStore::new(place).unwrap(), &stage("design"));
    assert!(tree.top().join(".yard/main/yardlog.yaml").is_file());
    assert!(!sub.join(".yard").exists());
}

#[test]
fn a_call_in_a_linked_worktree_reaches_that_worktree_s_own_yard_log() {
    let scratch = Scratch::new();
    let main = scratch.0.join("repo");
    let linked = scratch.0.join("wt");
    fs::create_dir_all(main.join(".git/worktrees/wt")).unwrap();
    fs::create_dir_all(&linked).unwrap();
    let common = main.join(".git");
    let scenario = GitScenario::clean_on("other").common_dir(
        &common.to_string_lossy(),
        &common.join("worktrees/wt").to_string_lossy(),
    );
    let plant = Scratch::new();
    let place = Place::locate(&crate::common::simulated(&scenario, &plant.0), &linked).unwrap();
    assert_eq!(place.tree, linked);
    assert_eq!(place.common_dir, common);
    assert_eq!(place.git_yard, common.join("worktrees/wt/yard"));
    write(&YardStore::new(place).unwrap(), &stage("design"));
    assert!(linked.join(".yard/other/yardlog.yaml").is_file());
    assert!(!main.join(".yard").exists());
}

#[test]
fn a_place_names_the_tree_the_branch_and_the_repository_s_common_directory() {
    let tree = Tree::new();
    let place = tree.place(&tree.on("main"), &tree.top()).unwrap();
    assert_eq!(place.tree, tree.top());
    assert_eq!(place.branch.as_deref(), Some("main"));
    assert_eq!(place.common_dir, tree.git_dir());
    assert_eq!(place.git_yard, tree.git_dir().join("yard"));
}

#[test]
fn a_write_on_a_detached_head_is_refused_and_writes_nothing_under_yard() {
    let tree = Tree::new();
    let place = tree
        .place(&tree.on("main").detached(), &tree.top())
        .unwrap();
    assert_eq!(place.branch, None);
    let error = YardStore::new(place).unwrap_err();
    assert!(matches!(error, YardStoreError::Detached(_)), "{error}");
    assert!(error.to_string().contains("belongs to a branch"), "{error}");
    assert!(!tree.top().join(".yard").exists());
}

#[test]
fn a_call_outside_any_repository_is_refused_as_needing_one() {
    let tree = Tree::new();
    let error = tree
        .place(&GitScenario::outside_repository(), &tree.top())
        .unwrap_err();
    assert!(
        matches!(error, YardStoreError::NotARepository(_)),
        "{error}"
    );
    assert!(error.to_string().contains("Yard Log needs one"), "{error}");
    assert!(!tree.top().join(".yard").exists());
}

#[test]
fn a_renamed_branch_keeps_its_yard_log_s_key_and_records_its_new_name() {
    let tree = Tree::new();
    write(&tree.store("feat/old"), &stage("design"));
    let key = read(&tree.store("feat/old")).root.key;
    fs::rename(
        tree.top().join(".yard/feat/old"),
        tree.top().join(".yard/feat/new"),
    )
    .unwrap();
    write(&tree.store("feat/new"), &stage("build"));
    let log = read(&tree.store("feat/new"));
    assert_eq!(log.root.key, key);
    assert_eq!(log.root.branch, "feat/new");
    assert_eq!(log.root.opened_as.as_deref(), Some("feat/old"));
}

#[test]
fn a_branch_name_that_cannot_name_a_folder_is_refused() {
    let tree = Tree::new();
    for branch in ["../x", "a//b", "a/./b"] {
        let place = Place {
            branch: Some(branch.to_owned()),
            ..tree.place(&tree.on("main"), &tree.top()).unwrap()
        };
        let error = YardStore::new(place).unwrap_err();
        assert!(
            matches!(error, YardStoreError::Branch(_)),
            "{branch}: {error}"
        );
    }
}

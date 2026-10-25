//! Concurrent writes: one write at a time is applied to a branch's Yard Log, under a lock in
//! git's own directory for the worktree that is held only while the write is applied, freed when
//! the process that holds it ends, and never shared with another branch's Yard Log; a write
//! interrupted part way is finished or undone by the next.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, fs, thread};

use crate::common::{Failed, HELPER_ENV, HELPER_TEST, Tree, holding, read, wait_for, write};
use loft_ports::journal::{Changes, RelPath};
use serde_json::json;
use yardlog_store::lock::{self, is_held};
use yardlog_store::{Place, YardStore};

fn note(id: &str) -> serde_json::Value {
    json!([{"id": id, "kind": "note", "stage": "design", "markdown": id}])
}

fn designed(tree: &Tree) -> YardStore {
    let store = tree.store("main");
    write(
        &store,
        &json!([{"id": "design", "kind": "stage", "title": "Design"}]),
    );
    store
}

#[test]
fn writes_that_come_at_once_from_many_agents_are_each_applied_one_after_the_other() {
    let tree = Tree::new();
    let store = designed(&tree);
    thread::scope(|scope| {
        for n in 0..8 {
            let store = store.clone();
            scope.spawn(move || write(&store, &note(&format!("thread-{n}"))));
        }
    });
    let log = read(&store);
    assert_eq!(log.root.revision, 9);
    assert_eq!(log.stages[0].blocks.len(), 8);
}

#[test]
fn writes_from_two_processes_at_once_are_each_applied() {
    let tree = Tree::new();
    let store = designed(&tree);
    let ready = tree.scratch.0.join("ready");
    let place = store.place();
    let mut child = Command::new(env::current_exe().unwrap())
        .args([HELPER_TEST, "--exact", "--include-ignored", "--nocapture"])
        .env(
            HELPER_ENV,
            format!(
                "write\n{}\n{}\n{}",
                place.tree.display(),
                place.git_yard.display(),
                ready.display()
            ),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    for n in 0..10 {
        write(&store, &note(&format!("parent-{n}")));
    }
    assert!(child.wait().unwrap().success());
    wait_for(&ready);
    let log = read(&store);
    assert_eq!(log.root.revision, 21);
    assert_eq!(log.stages[0].blocks.len(), 20);
}

#[test]
fn the_lock_and_the_journal_lie_under_git_s_own_directory_for_the_worktree() {
    let tree = Tree::new();
    let store = designed(&tree);
    let yard = tree.git_dir().join("yard");
    assert!(
        store.lock_path().starts_with(&yard),
        "{}",
        store.lock_path().display()
    );
    assert!(store.journal().dir().starts_with(&yard));
    let outside_git: Vec<String> = tree
        .scratch
        .snapshot()
        .into_keys()
        .filter(|path| !path.starts_with("tree/.git"))
        .filter(|path| !path.starts_with("tree/.yard/main/"))
        .collect();
    assert_eq!(outside_git, ["tree/", "tree/.yard/"]);
    assert!(store.lock_path().is_file());
}

#[test]
fn the_lock_is_held_while_a_write_is_applied_and_only_then() {
    let tree = Tree::new();
    let store = designed(&tree);
    assert!(!is_held(&store.lock_path()).unwrap());
    store
        .update(|log| -> Result<_, Failed> {
            assert!(is_held(&store.lock_path()).unwrap());
            Ok((log.unwrap(), ()))
        })
        .unwrap();
    assert!(!is_held(&store.lock_path()).unwrap());
}

#[test]
fn a_write_to_one_yard_log_never_waits_on_the_lock_of_another() {
    let tree = Tree::new();
    let main = designed(&tree);
    let other = tree.store("other");
    let ready = tree.scratch.0.join("ready");
    let mut child = holding(&main.lock_path(), &ready);
    assert!(is_held(&main.lock_path()).unwrap());
    let (done, finished) = std::sync::mpsc::channel();
    thread::spawn(move || {
        write(
            &other,
            &json!([{"id": "build", "kind": "stage", "title": "Build"}]),
        );
        done.send(()).unwrap();
    });
    let result = finished.recv_timeout(Duration::from_secs(20));
    child.kill().unwrap();
    child.wait().unwrap();
    result.expect("the write to the other branch's Yard Log finished");
}

#[test]
fn a_lock_left_by_a_process_that_ended_is_taken_over_by_the_next_write() {
    let tree = Tree::new();
    let store = designed(&tree);
    let ready = tree.scratch.0.join("ready");
    let mut child = holding(&store.lock_path(), &ready);
    assert!(is_held(&store.lock_path()).unwrap());
    child.kill().unwrap();
    child.wait().unwrap();
    write(&store, &note("after"));
    assert_eq!(read(&store).root.revision, 2);
}

/// The changes of a write of `log`'s next revision, with one more note, as a save would make them.
fn next_revision(store: &YardStore) -> Changes {
    let mut next = read(store);
    next.write(
        &yardlog::input::read_batch(&note("interrupted")).unwrap(),
        &yardlog::write::Context {
            at: crate::common::at(100),
            agent: crate::common::agent(),
            base: None,
            send: false,
        },
        &yardlog::write::Reports::new(),
    )
    .unwrap();
    let mut changes = Changes::default();
    for (name, text) in next.files() {
        changes.write.insert(
            RelPath::new(format!("{}/{name}", store.folder_rel())),
            text.into_bytes(),
        );
    }
    changes
}

#[test]
fn a_write_interrupted_after_its_commit_is_finished_by_the_next_write() {
    let tree = Tree::new();
    let store = designed(&tree);
    drop(
        store
            .journal()
            .prepare(&next_revision(&store))
            .unwrap()
            .commit()
            .unwrap(),
    );
    write(&store, &note("next"));
    let log = read(&store);
    assert_eq!(log.root.revision, 3);
    let ids: Vec<&str> = log.stages[0]
        .blocks
        .iter()
        .map(|block| block.id.as_str())
        .collect();
    assert_eq!(ids, ["interrupted", "next"]);
    assert!(!store.journal().dir().exists());
}

#[test]
fn a_write_interrupted_before_its_commit_is_undone_by_the_next_write() {
    let tree = Tree::new();
    let store = designed(&tree);
    drop(store.journal().prepare(&next_revision(&store)).unwrap());
    write(&store, &note("next"));
    let log = read(&store);
    assert_eq!(log.root.revision, 2);
    let ids: Vec<&str> = log.stages[0]
        .blocks
        .iter()
        .map(|block| block.id.as_str())
        .collect();
    assert_eq!(ids, ["next"]);
}

#[test]
fn a_read_finishes_a_write_interrupted_after_its_commit() {
    let tree = Tree::new();
    let store = designed(&tree);
    drop(
        store
            .journal()
            .prepare(&next_revision(&store))
            .unwrap()
            .commit()
            .unwrap(),
    );
    assert_eq!(read(&store).root.revision, 2);
}

/// What this test binary does when another test runs it again as a helper process: hold a lock
/// until it is killed, or write ten notes to Yard Log alongside its parent. Run alone, it does
/// nothing.
#[test]
#[ignore = "run by the tests that need a second process"]
fn helper() {
    let Ok(task) = env::var(HELPER_ENV) else {
        return;
    };
    let lines: Vec<&str> = task.lines().collect();
    match lines.as_slice() {
        ["hold", lock, ready] => {
            let _held = lock::lock(Path::new(lock)).unwrap();
            fs::write(ready, "").unwrap();
            // Killed by its parent; gone by itself should the parent fail first.
            thread::sleep(Duration::from_secs(60));
        }
        ["write", tree, git_yard, ready] => {
            let place = Place {
                tree: PathBuf::from(tree),
                branch: Some("main".to_owned()),
                common_dir: PathBuf::from(git_yard).join(".."),
                git_yard: PathBuf::from(git_yard),
            };
            let store = YardStore::new(place).unwrap();
            for n in 0..10 {
                write(&store, &note(&format!("child-{n}")));
            }
            fs::write(ready, "").unwrap();
        }
        other => panic!("an unknown task {other:?}"),
    }
}

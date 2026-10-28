//! What belongs to the machine and not to a branch, in `~/.yard/`: each file named as Yard
//! Client names it, written to a temporary file and renamed into place, the control token
//! readable only by the person's own account, and a change that reads a file and writes it back
//! made one at a time.

use std::thread;

use serde_json::json;
use yardlog::Lineage;
use yardlog_store::Held;
use yardlog_store::home::{Home, HomeFile};

use crate::common::{Failed, Scratch, Tree, write};

#[test]
fn each_file_of_the_machine_has_its_name_in_the_home_directory() {
    let scratch = Scratch::new();
    let home = Home::at(scratch.0.join(".yard"));
    let names = [
        (HomeFile::Server, "server.json"),
        (HomeFile::Trees, "trees.json"),
        (HomeFile::Sessions, "sessions.json"),
        (HomeFile::LastRead, "last-read.json"),
        (HomeFile::Continuations, "continuations.json"),
        (HomeFile::Token, "token"),
    ];
    for (file, name) in names {
        assert_eq!(home.path(file), scratch.0.join(".yard").join(name));
    }
    assert_eq!(
        home.events("session-a"),
        scratch.0.join(".yard/events/session-a.jsonl")
    );
}

#[test]
fn the_home_directory_is_yard_in_the_person_s_home() {
    let Some(home) = Home::from_env() else {
        return;
    };
    assert_eq!(home.dir(), std::env::home_dir().unwrap().join(".yard"));
}

#[test]
fn a_file_is_written_whole_through_a_temporary_file_renamed_into_place() {
    let scratch = Scratch::new();
    let home = Home::at(scratch.0.join(".yard"));
    assert_eq!(home.read(HomeFile::Trees).unwrap(), None);
    home.write(HomeFile::Trees, b"{\"trees\":[]}\n").unwrap();
    home.write(HomeFile::Trees, b"{\"trees\":[\"/a\"]}\n")
        .unwrap();
    assert_eq!(
        home.read(HomeFile::Trees).unwrap().as_deref(),
        Some(&b"{\"trees\":[\"/a\"]}\n"[..])
    );
    let left: Vec<String> = scratch.snapshot().into_keys().collect();
    assert_eq!(left, [".yard/", ".yard/trees.json"]);
}

#[test]
fn a_file_that_cannot_be_renamed_into_place_leaves_no_temporary_file_and_the_old_file_as_it_was() {
    let scratch = Scratch::new();
    let home = Home::at(scratch.0.join(".yard"));
    // A directory where the file belongs: the rename into place fails.
    scratch.put(".yard/server.json/keep", "");
    assert!(home.write(HomeFile::Server, b"{}").is_err());
    let left: Vec<String> = scratch.snapshot().into_keys().collect();
    assert_eq!(
        left,
        [".yard/", ".yard/server.json/", ".yard/server.json/keep"]
    );
}

#[cfg(unix)]
#[test]
fn the_control_token_is_readable_only_by_the_person_s_own_account() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::new();
    let home = Home::at(scratch.0.join(".yard"));
    home.write(HomeFile::Token, b"secret").unwrap();
    let mode = std::fs::metadata(home.path(HomeFile::Token))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

/// Adds one to the count in `sessions.json`.
fn count_up(home: &Home) {
    home.update(HomeFile::Sessions, |bytes| {
        let count: u32 = bytes.map_or(0, |bytes| {
            String::from_utf8(bytes).unwrap().parse().unwrap()
        });
        Some((count + 1).to_string().into_bytes())
    })
    .unwrap();
}

#[test]
fn changes_that_read_a_file_and_write_it_back_are_made_one_at_a_time() {
    let scratch = Scratch::new();
    let home = Home::at(scratch.0.join(".yard"));
    thread::scope(|scope| {
        for _ in 0..8 {
            let home = home.clone();
            scope.spawn(move || (0..5).for_each(|_| count_up(&home)));
        }
    });
    assert_eq!(
        home.read(HomeFile::Sessions).unwrap().as_deref(),
        Some(&b"40"[..])
    );
}

#[test]
fn a_log_the_store_reads_carries_the_chains_of_continuations_its_home_records() {
    let tree = Tree::new();
    write(
        &tree.store("feat/x"),
        &json!([{"id": "build", "kind": "stage", "title": "Build", "status": "active"}]),
    );
    let home = Home::at(tree.scratch.0.join(".yard"));
    home.write(HomeFile::Continuations, br#"{"session-b": "session-a"}"#)
        .unwrap();
    let store = tree.store("feat/x").with_home(Some(home));
    let Held::Log(log) = store.read().unwrap() else {
        panic!("no Yard Log");
    };
    assert_eq!(log.lineage.first("claude-code", "session-b"), "session-a");
    let written = store
        .update(|log| {
            let log = log.unwrap();
            let first = log.lineage.first("claude-code", "session-b");
            Ok::<_, Failed>((log, first))
        })
        .unwrap();
    assert_eq!(written.value, "session-a");
    let Held::Log(alone) = tree.store("feat/x").with_home(None).read().unwrap() else {
        panic!("no Yard Log");
    };
    assert_eq!(alone.lineage, Lineage::default());
}

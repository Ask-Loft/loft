//! The files a session writes alone for a branch's Yard Log, its token totals, its activity and
//! its consultation: each live copy written whole in git's directory for the worktree to a
//! temporary file and renamed into place, outside Yard Log's lock, by a session id that cannot
//! reach outside its folder, and saved into Yard Log's folder by each write of Yard Log; and the
//! Yard Logs a worktree's `.yard/` holds.

use std::fs;

use serde_json::json;
use yardlog::codec::session::FOLDERS;
use yardlog_store::YardStoreError;
use yardlog_store::session::{SessionFile, branches, is_session_id};

use crate::common::{Tree, agent, at, holding, snapshot, write};

#[test]
fn a_session_s_live_copy_is_written_whole_in_git_s_directory_outside_yard_log_s_lock() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    write(
        &store,
        &json!([{ "id": "s", "kind": "stage", "title": "S" }]),
    );
    let worktree = snapshot(&tree.top().join(".yard"));
    let ready = tree.scratch.0.join("held");
    let mut holder = holding(&store.lock_path(), &ready);
    for file in SessionFile::ALL {
        store.write_session(file, "s1", b"session: s1\n").unwrap();
        let path = store.live_session_path(file, "s1");
        assert_eq!(fs::read(&path).unwrap(), b"session: s1\n");
        let left: Vec<String> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["s1.yaml"], "no temporary file is left");
    }
    holder.kill().unwrap();
    holder.wait().unwrap();
    assert_eq!(
        snapshot(&tree.top().join(".yard")),
        worktree,
        "no file of the working tree changes"
    );
}

#[test]
fn a_session_s_live_copy_lies_under_git_s_directory_by_the_branch_s_escaped_name() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    let live = tree.git_dir().join("yard/feat%2Fshelf.sessions");
    assert_eq!(store.live_dir(), live);
    assert_eq!(
        store.live_session_path(SessionFile::Tokens, "s1"),
        live.join("tokens/s1.yaml")
    );
    assert_eq!(
        store.live_session_path(SessionFile::Activity, "s1"),
        live.join("activity/s1.yaml")
    );
    assert_eq!(
        store.live_session_path(SessionFile::Consultation, "s1"),
        live.join("consultation/s1.yaml")
    );
    assert_eq!(
        store.session_path(SessionFile::Tokens, "s1"),
        store.folder().join("tokens/s1.yaml")
    );
}

#[test]
fn a_read_of_a_session_s_file_takes_its_live_copy_before_the_folder_s() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    tree.scratch
        .put("tree/.yard/feat/shelf/tokens/s1.yaml", "session: folder\n");
    tree.scratch.put(
        "tree/.yard/feat/shelf/tokens/s2.yaml",
        "session: only-folder\n",
    );
    store
        .write_session(SessionFile::Tokens, "s1", b"session: live\n")
        .unwrap();
    store
        .write_session(SessionFile::Tokens, "s3", b"session: only-live\n")
        .unwrap();
    assert_eq!(
        store.session_file(SessionFile::Tokens, "s1").unwrap(),
        b"session: live\n"
    );
    assert_eq!(
        store.session_file(SessionFile::Tokens, "s2").unwrap(),
        b"session: only-folder\n"
    );
    let all: Vec<(String, String)> = store
        .session_files(SessionFile::Tokens)
        .into_iter()
        .map(|(key, bytes)| (key, String::from_utf8(bytes).unwrap()))
        .collect();
    assert_eq!(
        all,
        [
            ("s1".to_owned(), "session: live\n".to_owned()),
            ("s2".to_owned(), "session: only-folder\n".to_owned()),
            ("s3".to_owned(), "session: only-live\n".to_owned()),
        ]
    );
}

#[test]
fn each_write_of_yard_log_saves_the_live_copies_that_differ_into_its_folder() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    write(
        &store,
        &json!([{ "id": "s", "kind": "stage", "title": "S" }]),
    );
    for file in SessionFile::ALL {
        store.write_session(file, "s1", b"session: s1\n").unwrap();
    }
    let written = write(
        &store,
        &json!([{ "id": "n", "kind": "note", "stage": "s", "markdown": "N" }]),
    )
    .written;
    for folder in FOLDERS {
        let path = format!(".yard/feat/shelf/{folder}/s1.yaml");
        assert!(written.contains(&path), "{path} in {written:?}");
        assert_eq!(fs::read(tree.top().join(&path)).unwrap(), b"session: s1\n");
    }
    let again = write(
        &store,
        &json!([{ "id": "n", "kind": "note", "stage": "s", "markdown": "M" }]),
    )
    .written;
    assert!(
        again.iter().all(|path| !path.contains("/tokens/")),
        "a live copy the folder already holds is not written again: {again:?}"
    );
}

#[test]
fn a_write_leaves_a_folder_s_session_file_that_no_live_copy_holds_as_it_is() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    write(
        &store,
        &json!([{ "id": "s", "kind": "stage", "title": "S" }]),
    );
    tree.scratch.put(
        "tree/.yard/feat/shelf/activity/merged.yaml",
        "session: merged\n",
    );
    let written = write(
        &store,
        &json!([{ "id": "n", "kind": "note", "stage": "s", "markdown": "N" }]),
    )
    .written;
    assert!(written.iter().all(|path| !path.contains("/activity/")));
    assert_eq!(
        fs::read(tree.top().join(".yard/feat/shelf/activity/merged.yaml")).unwrap(),
        b"session: merged\n"
    );
}

#[test]
fn a_live_copy_is_not_saved_over_a_folder_file_of_a_format_this_loft_does_not_write() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    write(
        &store,
        &json!([{ "id": "s", "kind": "stage", "title": "S" }]),
    );
    let newer = "format: yardtokens/99\nsession: s1\n";
    let other = "format: yardconsult/2\nsession: s1\n";
    tree.scratch
        .put("tree/.yard/feat/shelf/tokens/s1.yaml", newer);
    tree.scratch
        .put("tree/.yard/feat/shelf/consultation/s1.yaml", other);
    tree.scratch.put(
        "tree/.yard/feat/shelf/tokens/s2.yaml",
        "format: yardtokens/1\nsession: s2\n",
    );
    for session in ["s1", "s2"] {
        for file in [SessionFile::Tokens, SessionFile::Consultation] {
            let text = format!("format: yardtokens/2\nsession: {session}\n");
            store.write_session(file, session, text.as_bytes()).unwrap();
        }
    }
    write(
        &store,
        &json!([{ "id": "n", "kind": "note", "stage": "s", "markdown": "N" }]),
    );
    let folder = |path: &str| fs::read_to_string(store.folder().join(path)).unwrap();
    assert_eq!(folder("tokens/s1.yaml"), newer);
    assert_eq!(folder("consultation/s1.yaml"), other);
    assert_eq!(
        folder("tokens/s2.yaml"),
        "format: yardtokens/2\nsession: s2\n",
        "an older totals format is written over"
    );
}

#[test]
fn closing_yard_log_saves_the_live_copies_then_removes_them() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    write(
        &store,
        &json!([{ "id": "s", "kind": "stage", "title": "S" }]),
    );
    store
        .write_session(SessionFile::Activity, "s1", b"session: s1\n")
        .unwrap();
    store
        .update(|log| {
            let mut log = log.unwrap();
            log.close(agent(), at(9)).unwrap();
            Ok::<_, YardStoreError>((log, ()))
        })
        .unwrap();
    assert_eq!(
        fs::read(store.session_path(SessionFile::Activity, "s1")).unwrap(),
        b"session: s1\n"
    );
    assert!(!store.live_dir().exists());
}

#[test]
fn a_session_id_that_could_reach_outside_its_folder_names_no_file() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    for session in ["", ".hidden", "../up", "a/b", "a\\b"] {
        assert!(!is_session_id(session), "{session:?}");
        assert!(
            store
                .write_session(SessionFile::Tokens, session, b"x")
                .is_err(),
            "{session:?}"
        );
    }
    assert!(is_session_id("4f0c-session_1.2"));
    assert!(!store.folder().exists());
}

#[test]
fn the_branches_a_worktree_holds_are_the_folders_with_a_root() {
    let tree = Tree::new();
    for branch in ["main", "feat/shelf"] {
        write(
            &tree.store(branch),
            &json!([{ "id": "s", "kind": "stage", "title": "S" }]),
        );
    }
    tree.store("feat/shelf")
        .write_session(SessionFile::Tokens, "s1", b"session: s1\n")
        .unwrap();
    assert_eq!(branches(&tree.top()).unwrap(), ["feat/shelf", "main"]);
}

#[test]
fn the_search_for_branches_goes_no_deeper_into_a_folder_that_holds_a_root() {
    let tree = Tree::new();
    let store = tree.store("main");
    write(
        &store,
        &json!([{ "id": "s", "kind": "stage", "title": "S" }]),
    );
    // A root within a Yard Log's own folders is none of a branch's: git allows no branch
    // `main/below` beside `main`, so the search need not look there.
    let below = store.folder().join("below");
    fs::create_dir_all(&below).unwrap();
    fs::copy(
        store.folder().join(yardlog::codec::ROOT),
        below.join(yardlog::codec::ROOT),
    )
    .unwrap();
    assert_eq!(branches(&tree.top()).unwrap(), ["main"]);
}

#[test]
fn every_file_a_session_writes_alone_lies_in_a_folder_the_merge_driver_and_the_page_know() {
    assert_eq!(SessionFile::ALL.map(SessionFile::folder), FOLDERS);
}

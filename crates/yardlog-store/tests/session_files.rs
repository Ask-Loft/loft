//! The files a session writes alone in Yard Log's folder, its token totals and its activity:
//! each written whole to a temporary file and renamed into place, outside Yard Log's lock, by a
//! session id that cannot reach outside its folder; and the Yard Logs a worktree's `.yard/`
//! holds.

use std::fs;

use serde_json::json;
use yardlog_store::session::{SessionFile, branches, is_session_id};

use crate::common::{Tree, holding, write};

#[test]
fn a_session_s_file_is_written_whole_outside_yard_log_s_lock() {
    let tree = Tree::new();
    let store = tree.store("feat/shelf");
    write(
        &store,
        &json!([{ "id": "s", "kind": "stage", "title": "S" }]),
    );
    let ready = tree.scratch.0.join("held");
    let mut holder = holding(&store.lock_path(), &ready);
    for file in [SessionFile::Tokens, SessionFile::Activity] {
        store.write_session(file, "s1", b"session: s1\n").unwrap();
        let path = store.session_path(file, "s1");
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
        store.session_path(SessionFile::Tokens, "s1"),
        store.folder().join("tokens/s1.yaml")
    );
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

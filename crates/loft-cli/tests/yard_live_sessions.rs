//! The live copies of a session's files: the page server keeps them in the worktree's git
//! directory as the session's events arrive, so that no event changes the working tree, and each
//! write of Yard Log saves them into its folder.

use std::fs;

use serde_json::json;

use crate::snapshot::snapshot;
use crate::yard::{BRANCH, Yard};
use crate::yard_serve_consult::described;
use crate::yard_serve_events::{event, open, post, tokens};

/// A turn of session `s1`'s main agent, whose adapter counts consultations, that reads a file and
/// makes one model request.
fn a_turn(yard: &Yard) -> Vec<serde_json::Value> {
    let read = json!({ "tool": "Read", "call": "r1", "reads": ["code"] });
    let request = json!({ "model": "opus", "tokens": tokens(10, 0, 0, 2) });
    vec![
        described(yard, true),
        event(yard, "tool.start", 2, &read),
        event(yard, "request", 3, &request),
        event(yard, "turn.end", 4, &json!({})),
    ]
}

#[test]
fn a_session_s_events_leave_every_file_of_the_working_tree_as_it_was() {
    let (yard, served) = open();
    let before = snapshot(&yard.scratch.0);
    let reply = post(&served, &a_turn(&yard));
    assert_eq!(reply["rejected"], json!([]), "{reply}");
    assert_eq!(snapshot(&yard.scratch.0), before);
    for folder in ["tokens", "activity", "consultation"] {
        let live = yard.live(BRANCH).join(folder).join("s1.yaml");
        assert!(live.is_file(), "{} holds the live copy", live.display());
    }
}

#[test]
fn a_write_of_yard_log_after_a_session_s_events_saves_their_files_into_its_folder() {
    let (yard, served) = open();
    post(&served, &a_turn(&yard));
    assert!(!yard.folder(BRANCH).join("tokens").exists());
    let note = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "Noted."}]"#;
    let written = yard.write("s1", note, &[]);
    assert_eq!(written["outcome"], "written", "{written}");
    for folder in ["tokens", "activity", "consultation"] {
        let name = format!("{folder}/s1.yaml");
        assert_eq!(
            fs::read(yard.folder(BRANCH).join(&name)).unwrap(),
            fs::read(yard.live(BRANCH).join(&name)).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn the_page_shows_the_activity_and_totals_of_live_copies_the_folder_does_not_hold() {
    let (yard, served) = open();
    post(&served, &a_turn(&yard));
    let tree = yard.scratch.0.canonicalize().unwrap();
    let reply = served.get(&format!("/api/log?tree={}&branch={BRANCH}", tree.display()));
    assert_eq!(reply.status, 200);
    let read = reply.json();
    assert_eq!(read["activity"][0]["session"], "s1", "{read}");
    assert_eq!(read["tokenSums"]["all"]["input"], 10, "{read}");
    assert!(!yard.folder(BRANCH).join("activity").exists());
}

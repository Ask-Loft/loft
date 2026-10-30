//! The page server files each event under its agent's working tree: the worktree its session
//! wrote in whose Yard Log holds the agent's working stage, else the directory the event names.

use std::fs;

use serde_json::{Value, json};

use crate::common::Scratch;
use crate::yard::{BRANCH, Yard};
use crate::yard_serve_events::{event, log, open, post, session_file, tokens};

const THERE: &str = r#"[{"id": "there", "kind": "stage", "title": "There", "status": "active"}]"#;
const THERE_DONE: &str =
    r#"[{"id": "there", "kind": "stage", "title": "There", "status": "done"}]"#;

/// The top of `scratch` as events and the event log name it.
fn top(scratch: &Scratch) -> String {
    scratch.0.canonicalize().unwrap().display().to_string()
}

/// Opens the stage `there` in the worktree `there` for session `s1`'s main agent.
fn work_there(yard: &Yard, there: &Scratch, batch: &str) {
    let tree = there.0.display().to_string();
    let written = yard.write("s1", batch, &["--tree", &tree]);
    assert_eq!(written["outcome"], "written", "{written}");
}

/// Session `s1`'s tokens file in the Yard Log of the worktree `there`, as text.
fn tokens_there(there: &Scratch) -> Option<String> {
    let path = there.0.join(".yard").join(BRANCH).join("tokens/s1.yaml");
    fs::read_to_string(path).ok()
}

fn request(yard: &Yard, second: u32, input: u64) -> Value {
    let more = json!({ "model": "opus", "tokens": tokens(input, 0, 0, 1) });
    event(yard, "request", second, &more)
}

#[test]
fn a_main_agent_s_event_is_filed_under_the_worktree_that_holds_its_working_stage() {
    let (yard, served) = open();
    let there = Scratch::empty();
    work_there(&yard, &there, THERE);
    post(&served, &[request(&yard, 1, 7)]);
    let kept = log(&yard, "s1");
    assert_eq!(kept[0]["tree"], top(&there).as_str(), "{kept:?}");
    let text = tokens_there(&there).unwrap();
    assert!(text.contains("tokens:\n  input: 7\n"), "{text}");
    assert!(session_file(&yard, "tokens", "s1").is_none());
}

#[test]
fn the_events_of_a_turn_whose_working_stage_moves_are_split_where_it_moved() {
    let (yard, served) = open();
    let there = Scratch::empty();
    work_there(&yard, &there, THERE);
    post(
        &served,
        &[
            event(&yard, "turn.start", 0, &json!({})),
            request(&yard, 1, 3),
        ],
    );
    work_there(&yard, &there, THERE_DONE);
    post(&served, &[request(&yard, 2, 5)]);
    let trees: Vec<Value> = log(&yard, "s1")
        .iter()
        .map(|line| line["tree"].clone())
        .collect();
    let (theirs, ours) = (top(&there), top(&yard.scratch));
    assert_eq!(trees, [json!(theirs), json!(theirs), json!(ours)]);
    assert!(tokens_there(&there).unwrap().contains("  input: 3\n"));
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert!(text.contains("tokens:\n  input: 5\n"), "{text}");
}

#[test]
fn a_worktree_whose_yard_log_is_closed_never_takes_an_event() {
    let (yard, served) = open();
    let there = Scratch::empty();
    work_there(&yard, &there, THERE);
    let tree = there.0.display().to_string();
    yard.run(&["close", "--session", "s1", "--tree", &tree]);
    post(&served, &[request(&yard, 1, 2)]);
    assert_eq!(log(&yard, "s1")[0]["tree"], top(&yard.scratch).as_str());
}

#[test]
fn a_subagent_in_a_worktree_of_its_own_files_its_events_there_while_its_main_agent_works_elsewhere()
{
    let (yard, served) = open();
    let (there, own) = (Scratch::empty(), Scratch::empty());
    let planned = r#"[{"id": "later", "kind": "stage", "title": "Later", "status": "planned"}]"#;
    work_there(&yard, &own, planned);
    work_there(&yard, &there, THERE);
    let mut sub = request(&yard, 1, 4);
    sub["dir"] = json!(top(&own));
    sub["agent"] = json!({ "id": "a1", "type": "Explore" });
    let main = event(&yard, "turn.start", 0, &json!({}));
    post(&served, &[main, sub]);
    let kept = log(&yard, "s1");
    assert_eq!(kept[0]["tree"], top(&there).as_str(), "{kept:?}");
    assert_eq!(kept[1]["tree"], top(&own).as_str(), "{kept:?}");
}

#[test]
fn a_main_agent_s_event_records_its_directory_for_its_subagents() {
    let (yard, served) = open();
    let elsewhere = Scratch::empty();
    let mut main = event(&yard, "turn.start", 0, &json!({}));
    main["dir"] = json!(top(&elsewhere));
    post(&served, &[main]);
    assert_eq!(
        yard.home_file("session-dirs.json"),
        json!({ "s1": top(&elsewhere) })
    );
}

//! An agent's work follows its working stage into another worktree: the `loft yard` commands
//! that act for an agent act on its working tree, `--tree` names another worktree, a submission
//! is found where it was sent, and each write records its worktree in `~/.yard/`.

use std::fs;

use serde_json::{Value, json};

use crate::common::Scratch;
use crate::yard::{ASK, BRANCH, Yard, block, document};
use crate::yard_wait::wait;

const THERE: &str = r#"[{"id": "there", "kind": "stage", "title": "There", "status": "active"}]"#;
const THERE_DONE: &str =
    r#"[{"id": "there", "kind": "stage", "title": "There", "status": "done"}]"#;
const HERE: &str = r#"[{"id": "here", "kind": "stage", "title": "Here", "status": "active"}]"#;
const NOTE: &str =
    r#"[{"id": "n1", "kind": "note", "stage": "there", "markdown": "Written without a tree"}]"#;

/// A second worktree beside the scratch repository, of another repository.
fn other() -> Scratch {
    Scratch::empty()
}

/// The top of `scratch` as results name it.
fn top(scratch: &Scratch) -> String {
    scratch.0.canonicalize().unwrap().display().to_string()
}

fn path(scratch: &Scratch) -> String {
    scratch.0.display().to_string()
}

const SUB: [&str; 4] = ["--agent", "a1", "--agent-type", "Explore"];

/// Reads Yard Log as session s1's subagent `a1`, run from `dir`: the tree it acted on.
fn subagent_reads_from(yard: &Yard, dir: &Scratch) -> Value {
    let mut command = yard.scratch.command(Some(&yard.git), &yard.clock);
    let output = command
        .current_dir(&dir.0)
        .args(["yard", "read", "--session", "s1"])
        .args(SUB)
        .args(["--format", "json"])
        .output()
        .unwrap();
    document("read", &output)["tree"].clone()
}

/// The tree a read as `session` with `extra` arguments acts on.
fn read_tree(yard: &Yard, session: &str, extra: &[&str]) -> Value {
    yard.read(session, extra)["tree"].clone()
}

#[test]
fn a_session_s_calls_follow_its_working_stage_into_another_worktree() {
    let yard = Yard::new();
    let there = other();
    let opened = yard.write("s1", THERE, &["--tree", &path(&there)]);
    assert_eq!(opened["tree"], top(&there).as_str(), "{opened}");
    assert_eq!(read_tree(&yard, "s1", &[]), top(&there).as_str());
    let written = yard.write("s1", NOTE, &[]);
    assert_eq!(written["outcome"], "written", "{written}");
    assert_eq!(written["tree"], top(&there).as_str());
    assert!(
        there
            .0
            .join(".yard")
            .join(BRANCH)
            .join("yardlog.yaml")
            .is_file()
    );
    assert!(!yard.folder(BRANCH).exists());
}

#[test]
fn an_agent_that_ends_its_working_stage_in_another_worktree_works_in_its_own_directory_again() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    yard.write("s1", THERE_DONE, &["--tree", &path(&there)]);
    assert_eq!(read_tree(&yard, "s1", &[]), top(&yard.scratch).as_str());
}

#[test]
fn an_agent_with_no_working_stage_anywhere_works_in_its_own_directory() {
    let yard = Yard::new();
    assert_eq!(read_tree(&yard, "s1", &[]), top(&yard.scratch).as_str());
}

#[test]
fn of_two_worktrees_holding_a_working_stage_the_one_the_agent_wrote_in_last_wins() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", HERE, &["--tree", &path(&yard.scratch)]);
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    assert_eq!(read_tree(&yard, "s1", &[]), top(&there).as_str());
    let note = r#"[{"id": "n2", "kind": "note", "stage": "here", "markdown": "Back"}]"#;
    yard.write("s1", note, &["--tree", &path(&yard.scratch)]);
    assert_eq!(read_tree(&yard, "s1", &[]), top(&yard.scratch).as_str());
}

#[test]
fn a_worktree_whose_yard_log_is_closed_is_never_a_working_tree() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    let closed = yard.run(&["close", "--session", "s1", "--tree", &path(&there)]);
    assert_eq!(document("close", &closed)["outcome"], "closed");
    assert_eq!(read_tree(&yard, "s1", &[]), top(&yard.scratch).as_str());
}

#[test]
fn a_worktree_that_no_longer_exists_is_never_a_working_tree_and_is_dropped_from_the_record() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    let gone = top(&there);
    drop(there);
    assert_eq!(read_tree(&yard, "s1", &[]), top(&yard.scratch).as_str());
    yard.write("s1", HERE, &[]);
    let record = yard.home_file("session-trees.json");
    assert!(record["s1"].get(top(&yard.scratch)).is_some(), "{record}");
    assert!(record["s1"].get(&gone).is_none(), "{record}");
}

#[test]
fn a_subagent_follows_its_own_working_stage_with_its_own_directory_as_the_fallback() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    let sub = ["--agent", "a1", "--agent-type", "Explore"];
    // With no stage of its own, the subagent works in its main agent's working stage.
    assert_eq!(read_tree(&yard, "s1", &sub), top(&there).as_str());
    let own = r#"[{"id": "own", "kind": "stage", "title": "Own", "status": "active"}]"#;
    let tree = path(&yard.scratch);
    let opened = yard.write(
        "s1",
        own,
        &[sub[0], sub[1], sub[2], sub[3], "--tree", &tree],
    );
    assert_eq!(opened["outcome"], "written", "{opened}");
    assert_eq!(read_tree(&yard, "s1", &sub), top(&yard.scratch).as_str());
    assert_eq!(read_tree(&yard, "s1", &[]), top(&there).as_str());
}

#[test]
fn a_session_that_continues_another_searches_the_worktrees_the_ended_one_wrote_in() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    yard.run(&["continue", "--session", "s2", "--from", "s1"]);
    assert_eq!(read_tree(&yard, "s2", &[]), top(&there).as_str());
}

#[test]
fn each_write_records_its_worktree_and_time_for_the_session_and_a_read_records_nothing() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    let record = yard.home_file("session-trees.json");
    let write = &record["s1"][top(&there).as_str()]["main"];
    assert_eq!(write["at"], crate::yard::EPOCH_TEXT, "{record}");
    assert!(write["order"].is_u64(), "{record}");
    let file = yard.scratch.home().join(".yard/session-trees.json");
    let before = fs::read(&file).unwrap();
    yard.read("s1", &["--mark-read"]);
    assert_eq!(fs::read(&file).unwrap(), before);
}

#[test]
fn a_task_change_acts_on_the_working_tree() {
    let yard = Yard::new();
    let there = other();
    let batch = r#"[{"id": "there", "kind": "stage", "title": "There", "status": "active"},
      {"id": "t1", "kind": "task", "stage": "there", "title": "One", "state": "open"}]"#;
    yard.write("s1", batch, &["--tree", &path(&there)]);
    let changed = yard.run(&["task", "t1", "--state", "active", "--session", "s1"]);
    let changed = document("task", &changed);
    assert_eq!(changed["tree"], top(&there).as_str(), "{changed}");
}

#[test]
fn close_and_published_stay_in_their_directory_whatever_the_working_tree() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", HERE, &[]);
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    let closed = document("close", &yard.run(&["close", "--session", "s1"]));
    assert_eq!(closed["tree"], top(&yard.scratch).as_str(), "{closed}");
    let open = yard.read("s1", &["--tree", &path(&there)]);
    assert_eq!(open["yardLog"]["closedBy"], "open", "{open}");
    let published = yard.run(&["published", "--session", "s1", "--branch", "feat/gone"]);
    let published = document("published", &published);
    assert_eq!(
        published["tree"],
        top(&yard.scratch).as_str(),
        "{published}"
    );
}

#[test]
fn a_command_run_without_session_acts_on_its_own_directory() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    let read = document("read", &yard.run(&["read"]));
    assert_eq!(read["tree"], top(&yard.scratch).as_str());
}

#[test]
fn the_briefing_describes_the_working_tree_s_yard_log_and_names_it() {
    let yard = Yard::new();
    let there = other();
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    let read = yard.read("s1", &["--context"]);
    let context = &read["context"];
    assert_eq!(context["tree"], top(&there).as_str(), "{read}");
    assert_eq!(context["stage"]["id"], "there", "{read}");
    assert!(context["text"].as_str().unwrap().contains(&top(&there)));
}

#[test]
fn a_tree_that_is_no_directory_is_refused_naming_it_and_nothing_is_written() {
    let yard = Yard::new();
    let missing = yard.scratch.0.join("no-such-place");
    let output = yard.run_with(
        &[
            "write",
            "--session",
            "s1",
            "--tree",
            missing.to_str().unwrap(),
        ],
        THERE,
    );
    assert_eq!(output.status.code(), Some(1));
    let refused = document("write", &output);
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("no-such-place"), "{refused}");
    assert!(!yard.folder(BRANCH).exists());
    assert!(
        !yard
            .scratch
            .home()
            .join(".yard/session-trees.json")
            .exists()
    );
}

#[test]
fn a_write_given_a_tree_opens_that_worktree_s_yard_log() {
    let yard = Yard::new();
    let there = other();
    let opened = yard.write("s1", THERE, &["--tree", &path(&there)]);
    assert_eq!(opened["revision"], 1, "{opened}");
    assert_eq!(opened["branch"], BRANCH);
    assert!(
        there
            .0
            .join(".yard")
            .join(BRANCH)
            .join("yardlog.yaml")
            .is_file()
    );
}

/// Sends [`ASK`]'s question from the worktree `there`, then ends its stage there, so that the
/// session's working tree is its own directory again; the submission's key.
fn sent_there_then_left(yard: &Yard, there: &Scratch) -> String {
    let sent = yard.write("s1", ASK, &["--tree", &path(there), "--send"]);
    let done = r#"[{"id": "shape", "kind": "stage", "title": "Shape it", "status": "done"}]"#;
    yard.write("s1", done, &["--tree", &path(there)]);
    assert_eq!(read_tree(yard, "s1", &[]), top(&yard.scratch).as_str());
    sent["submission"].as_str().unwrap().to_owned()
}

#[test]
fn a_wait_on_a_submission_sent_to_another_worktree_reads_it_there() {
    let yard = Yard::new();
    let there = other();
    let submission = sent_there_then_left(&yard, &there);
    let (code, waited) = wait(&yard, "s1", &submission, &["--timeout", "0"]);
    assert_eq!(code, Some(0), "{waited}");
    assert_eq!(waited["tree"], top(&there).as_str(), "{waited}");
}

#[test]
fn an_answer_in_the_dialog_is_recorded_in_the_yard_log_the_question_was_sent_to() {
    let yard = Yard::new();
    let there = other();
    sent_there_then_left(&yard, &there);
    let answered = yard.answer("s1", r#"[{"block": "q1", "labels": ["Top"]}]"#);
    assert_eq!(answered["tree"], top(&there).as_str(), "{answered}");
    let read = yard.read("s1", &["--tree", &path(&there)]);
    assert_eq!(block(&read, "q1")["answer"]["labels"], json!(["Top"]));
}

#[test]
fn a_withdrawal_reaches_the_submission_where_it_was_sent() {
    let yard = Yard::new();
    let there = other();
    let submission = sent_there_then_left(&yard, &there);
    let output = yard.run(&["withdraw", "--session", "s1", "--submission", &submission]);
    let withdrawn = document("withdraw", &output);
    assert_eq!(withdrawn["outcome"], "withdrawn", "{withdrawn}");
    assert_eq!(withdrawn["tree"], top(&there).as_str());
}

#[test]
fn a_submission_no_worktree_of_the_session_holds_is_refused_as_unknown() {
    let yard = Yard::new();
    let there = other();
    sent_there_then_left(&yard, &there);
    yard.write("s1", HERE, &[]);
    let unknown = "01a18a56-c26c-76b9-ba1b-4833d1999c0a";
    let output = yard.run(&["withdraw", "--session", "s1", "--submission", unknown]);
    assert_eq!(output.status.code(), Some(1));
    let refused = document("withdraw", &output);
    assert_eq!(refused["errors"][0]["kind"], "no-submission", "{refused}");
}

#[test]
fn a_subagent_in_a_worktree_of_its_own_with_nothing_of_its_own_stays_there() {
    let yard = Yard::new();
    let (there, own) = (other(), other());
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    assert_eq!(subagent_reads_from(&yard, &own), top(&own).as_str());
}

#[test]
fn a_subagent_in_a_worktree_of_its_own_follows_a_task_assigned_to_it_elsewhere() {
    let yard = Yard::new();
    let (there, own) = (other(), other());
    let batch = r#"[{"id": "there", "kind": "stage", "title": "There", "status": "active"},
      {"id": "t1", "kind": "task", "stage": "there", "title": "One", "state": "active",
       "assignee": {"host": "claude-code", "session": "s1", "agent": "a1", "agentType": "Explore"}}]"#;
    yard.write("s1", batch, &["--tree", &path(&there)]);
    assert_eq!(subagent_reads_from(&yard, &own), top(&there).as_str());
}

#[test]
fn a_subagent_whose_main_agent_s_directory_is_not_known_follows_its_main_agent() {
    let yard = Yard::new();
    let (there, own) = (other(), other());
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    fs::remove_file(yard.scratch.home().join(".yard/session-dirs.json")).unwrap();
    assert_eq!(subagent_reads_from(&yard, &own), top(&there).as_str());
}

#[test]
fn a_main_agent_s_command_records_its_directory_and_a_subagent_s_does_not() {
    let yard = Yard::new();
    let (there, own) = (other(), other());
    yard.write("s1", THERE, &["--tree", &path(&there)]);
    let dirs = yard.home_file("session-dirs.json");
    assert_eq!(dirs, json!({ "s1": top(&yard.scratch) }));
    subagent_reads_from(&yard, &own);
    assert_eq!(yard.home_file("session-dirs.json"), dirs);
}

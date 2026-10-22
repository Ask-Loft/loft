//! `loft yard withdraw` and `loft yard close` as processes: a submission withdrawn and its
//! blocks left unanswered; Yard Log closed with the Act of the close, taking no more writes or
//! answers; and Yard Log the base branch holds, read as closed by the merge.

use serde_json::json;

use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, EPOCH_TEXT, Yard, block, document};

const TWO: &str = r#"[
  {"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"},
  {"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which shelf?", "header": "Shelf",
   "options": [{"label": "Top"}, {"label": "Bottom"}]},
  {"id": "q2", "kind": "question", "stage": "shape", "prompt": "Which spine?", "header": "Spine",
   "options": [{"label": "Red"}, {"label": "Blue"}]}
]"#;

fn withdraw(yard: &Yard, session: &str, submission: &str) -> std::process::Output {
    yard.run(&["withdraw", "--session", session, "--submission", submission])
}

#[test]
fn a_withdrawal_keeps_the_answers_given_before_and_records_when_and_by_whom() {
    let yard = Yard::new();
    let submission = yard.write("s1", TWO, &["--send"])["submission"].clone();
    let submission = submission.as_str().unwrap();
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Top"]}]"#);
    let output = withdraw(&yard, "s2", submission);
    let withdrawn = document("withdraw", &output);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(withdrawn["outcome"], "withdrawn");
    assert_eq!(withdrawn["revision"], 3);
    let read = yard.read("s1", &[]);
    assert_eq!(read["submissions"], json!([]));
    assert_eq!(block(&read, "q1")["answer"]["labels"], json!(["Top"]));
    assert!(block(&read, "q2").get("answer").is_none());
    let root = std::fs::read_to_string(yard.folder(BRANCH).join("yardlog.yaml")).unwrap();
    assert!(root.contains("what: withdrawn"), "{root}");
    assert!(root.contains("session: \"s2\""), "{root}");
}

#[test]
fn withdrawing_a_submission_that_is_not_open_is_refused_with_its_state_and_changes_no_file() {
    let yard = Yard::new();
    let submission = yard.write("s1", ASK, &["--send"])["submission"].clone();
    let submission = submission.as_str().unwrap();
    withdraw(&yard, "s1", submission);
    let before = snapshot(&yard.scratch.0);
    let output = withdraw(&yard, "s1", submission);
    let refused = document("withdraw", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(refused["errors"][0]["kind"], "not-open");
    assert!(
        refused["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("withdrawn")
    );
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn closed_yard_log_says_when_and_by_which_agent_and_reads_as_before() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let before = yard.read("s1", &[]);
    let output = yard.run(&["close", "--session", "s2"]);
    assert_eq!(document("close", &output)["outcome"], "closed");
    let after = yard.read("s1", &[]);
    assert_eq!(after["yardLog"]["closedBy"], "agent");
    assert_eq!(after["yardLog"]["closed"]["at"], EPOCH_TEXT);
    assert_eq!(after["yardLog"]["closed"]["by"]["session"], "s2");
    assert_eq!(after["stages"], before["stages"]);
}

#[test]
fn closing_withdraws_every_open_submission() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let other = r#"[{"id": "q9", "kind": "question", "stage": "shape", "prompt": "Why?",
        "header": "Why", "options": [{"label": "A"}, {"label": "B"}]}]"#;
    yard.write("s2", other, &["--send"]);
    yard.run(&["close", "--session", "s1"]);
    assert_eq!(yard.read("s1", &[])["submissions"], json!([]));
}

#[test]
fn a_batch_or_an_answer_for_closed_yard_log_is_refused_and_changes_no_file() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    yard.run(&["close", "--session", "s1"]);
    let before = snapshot(&yard.scratch.0);
    let output = yard.run_with(&["write", "--session", "s1"], ASK);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(document("write", &output)["errors"][0]["kind"], "closed");
    let output = yard.run_with(
        &["answer", "--session", "s1"],
        r#"[{"block": "q1", "labels": ["Top"]}]"#,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn yard_log_whose_root_the_base_branch_holds_reads_as_closed_by_the_merge() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    assert_eq!(yard.read("s1", &[])["yardLog"]["closedBy"], "open");
    let root = format!(".yard/{BRANCH}/yardlog.yaml");
    yard.git = yard.git.clone().holding("develop", &root, "2222222");
    assert_eq!(yard.read("s1", &[])["yardLog"]["closedBy"], "merge");
}

#[test]
fn writes_and_answers_to_yard_log_closed_by_the_merge_are_refused_as_for_closed_yard_log() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let root = format!(".yard/{BRANCH}/yardlog.yaml");
    yard.git = yard.git.clone().holding("develop", &root, "2222222");
    let before = snapshot(&yard.scratch.0);
    let output = yard.run_with(&["write", "--session", "s1"], ASK);
    assert_eq!(document("write", &output)["errors"][0]["kind"], "closed");
    let output = yard.run_with(
        &["answer", "--session", "s1"],
        r#"[{"block": "q1", "labels": ["Top"]}]"#,
    );
    assert_eq!(document("answer", &output)["errors"][0]["kind"], "closed");
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn yard_log_the_base_branch_does_not_hold_stays_open_whatever_its_branch_holds() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let root = format!(".yard/{BRANCH}/yardlog.yaml");
    yard.git = yard.git.clone().holding(BRANCH, &root, "3333333");
    assert_eq!(yard.read("s1", &[])["yardLog"]["closedBy"], "open");
    assert_eq!(yard.write("s1", ASK, &[])["outcome"], "written");
}

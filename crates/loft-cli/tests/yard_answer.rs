//! `loft yard answer` as a process: the person's answers and decisions given in the agent
//! host's own dialog, recorded with the person git names and the channel `terminal`, all at
//! one revision; the first answer kept where a block was answered first; and a run refused
//! whole where a block is in no open submission.

use serde_json::json;

use crate::snapshot::snapshot;
use crate::yard::{EPOCH_TEXT, Yard, block, document};

/// A sent batch of two questions, an approval and a plan.
const ASKED: &str = r#"[
  {"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"},
  {"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which shelf?", "header": "Shelf",
   "options": [{"label": "Top"}, {"label": "Bottom"}], "ownWords": true},
  {"id": "q2", "kind": "question", "stage": "shape", "prompt": "Which spine?", "header": "Spine",
   "options": [{"label": "Red"}, {"label": "Blue"}]},
  {"id": "go", "kind": "approval", "stage": "shape", "title": "Order it", "markdown": "Order it."},
  {"id": "how", "kind": "plan", "stage": "shape", "title": "The plan", "markdown": "1. Order"}
]"#;

fn asked() -> Yard {
    let yard = Yard::new();
    yard.write("s1", ASKED, &["--send"]);
    yard
}

#[test]
fn an_answer_records_the_labels_or_the_person_s_own_words_and_the_person_s_notes() {
    let yard = asked();
    yard.answer(
        "s1",
        r#"[{"block": "q1", "words": "The middle one", "notes": "It fits"},
            {"block": "q2", "labels": ["Red"]}]"#,
    );
    let read = yard.read("s1", &[]);
    let q1 = &block(&read, "q1")["answer"];
    assert_eq!(q1["words"], "The middle one");
    assert_eq!(q1["notes"], "It fits");
    assert_eq!(block(&read, "q2")["answer"]["labels"], json!(["Red"]));
}

#[test]
fn a_decision_records_approved_or_rejected_with_the_person_s_reason() {
    let yard = asked();
    yard.answer(
        "s1",
        r#"[{"block": "go", "verdict": "approved"},
            {"block": "how", "verdict": "rejected", "reason": "Order later"}]"#,
    );
    let read = yard.read("s1", &[]);
    assert_eq!(block(&read, "go")["outcome"]["verdict"], "approved");
    let how = &block(&read, "how")["outcome"];
    assert_eq!(how["verdict"], "rejected");
    assert_eq!(how["reason"], "Order later");
    assert_eq!(how["channel"], "terminal");
}

#[test]
fn the_person_is_named_by_the_user_name_and_user_email_git_gives() {
    let yard = asked();
    yard.answer("s1", r#"[{"block": "go", "verdict": "approved"}]"#);
    let read = yard.read("s1", &[]);
    let outcome = &block(&read, "go")["outcome"];
    assert_eq!(
        outcome["act"]["by"],
        json!({ "name": "Ada Reader", "email": "ada@example.com" })
    );
    assert_eq!(outcome["act"]["at"], EPOCH_TEXT);
}

#[test]
fn all_the_answers_of_one_run_are_recorded_at_one_revision() {
    let yard = asked();
    let answered = yard.answer(
        "s1",
        r#"[{"block": "q1", "labels": ["Top"]}, {"block": "go", "verdict": "approved"}]"#,
    );
    assert_eq!(answered["revision"], 2);
    let read = yard.read("s1", &[]);
    assert_eq!(block(&read, "q1")["answer"]["act"]["revision"], 2);
    assert_eq!(block(&read, "go")["outcome"]["act"]["revision"], 2);
}

#[test]
fn a_submission_whose_last_open_block_is_answered_becomes_answered() {
    let yard = asked();
    let first = yard.answer(
        "s1",
        r#"[{"block": "q1", "labels": ["Top"]}, {"block": "q2", "labels": ["Red"]},
            {"block": "go", "verdict": "approved"}]"#,
    );
    assert_eq!(first["completed"], json!([]));
    let last = yard.answer("s1", r#"[{"block": "how", "verdict": "approved"}]"#);
    assert_eq!(last["completed"].as_array().unwrap().len(), 1);
    assert_eq!(yard.read("s1", &[])["submissions"], json!([]));
}

#[test]
fn a_block_answered_elsewhere_first_keeps_that_answer_and_the_others_are_recorded() {
    let yard = asked();
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Top"]}]"#);
    let output = yard.run_with(
        &["answer", "--session", "s1"],
        r#"[{"block": "q1", "labels": ["Bottom"]}, {"block": "q2", "labels": ["Blue"]}]"#,
    );
    let answered = document("answer", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(answered["outcome"], "answered elsewhere");
    assert_eq!(answered["elsewhere"][0]["id"], "q1");
    assert_eq!(answered["elsewhere"][0]["channel"], "terminal");
    assert_eq!(answered["errors"][0]["kind"], "answered-elsewhere");
    assert_eq!(answered["recorded"][0]["id"], "q2");
    let read = yard.read("s1", &[]);
    assert_eq!(block(&read, "q1")["answer"]["labels"], json!(["Top"]));
    assert_eq!(block(&read, "q2")["answer"]["labels"], json!(["Blue"]));
}

#[test]
fn a_run_with_a_block_in_no_open_submission_is_refused_and_changes_no_file() {
    let yard = Yard::new();
    yard.write("s1", ASKED, &[]);
    let before = snapshot(&yard.scratch.0);
    let output = yard.run_with(
        &["answer", "--session", "s1"],
        r#"[{"block": "q1", "labels": ["Top"]}]"#,
    );
    let refused = document("answer", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(refused["errors"][0]["kind"], "not-asked");
    assert!(
        refused["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("`q1`")
    );
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

//! The eight block kinds and what each holds.

use serde_json::{Value, json};
use yardlog::body::{Body, Status, TaskState};
use yardlog::input::{Fault, read_batch};

use crate::common::{context, main_agent, planned, question, write};

fn faults(batch: &Value) -> Vec<Fault> {
    read_batch(batch).expect_err("the batch is refused")
}

fn fields(found: &[Fault]) -> Vec<(&str, &str)> {
    found
        .iter()
        .map(|fault| (fault.block.as_str(), fault.field.as_str()))
        .collect()
}

#[test]
fn an_id_is_lowercase_letters_digits_and_hyphens_starting_with_a_letter() {
    for id in ["a", "stage-2", "q1"] {
        assert!(yardlog::Handle::parse(id).is_ok(), "{id}");
    }
    for id in ["", "2a", "-a", "A", "a_b", "a b", "é"] {
        assert!(yardlog::Handle::parse(id).is_err(), "{id}");
    }
    let found = faults(&json!([{"id": "Bad", "kind": "stage", "title": "S"}]));
    assert_eq!(fields(&found), [("Bad", "id")]);
}

#[test]
fn a_block_of_each_of_the_eight_kinds_is_read() {
    let batch = json!([
        {"id": "s", "kind": "stage", "title": "S"},
        {"id": "n", "kind": "note", "stage": "s", "markdown": "N"},
        question("q"),
        {"id": "a", "kind": "approval", "stage": "s", "title": "A", "markdown": "Approve?"},
        {"id": "d", "kind": "decision", "stage": "s", "title": "D", "consequence": "C"},
        {"id": "t", "kind": "task", "stage": "s", "title": "T"},
        {"id": "p", "kind": "plan", "stage": "s", "title": "P", "markdown": "Plan"},
        {"id": "l", "kind": "loft", "stage": "s", "view": "trace"}
    ]);
    assert_eq!(read_batch(&batch).unwrap().len(), 8);
}

#[test]
fn a_block_of_another_kind_is_refused() {
    let found = faults(&json!([{"id": "x", "kind": "memo", "stage": "s"}]));
    assert_eq!(fields(&found), [("x", "kind")]);
    assert!(found[0].message.contains("`stage`, `note`"), "{found:?}");
}

#[test]
fn a_block_other_than_a_stage_that_names_no_stage_is_refused() {
    let found = faults(&json!([{"id": "n", "kind": "note", "markdown": "N"}]));
    assert_eq!(fields(&found), [("n", "stage")]);
}

#[test]
fn a_block_that_names_a_stage_yard_log_does_not_hold_is_refused() {
    let mut log = planned();
    let refusal = crate::common::try_write(
        &mut log,
        &json!([{"id": "n2", "kind": "note", "stage": "build", "markdown": "N"}]),
        &context(&main_agent(), 2),
    )
    .unwrap_err();
    let yardlog::write::Refusal::Faults(found) = refusal else {
        panic!("{refusal:?}")
    };
    assert_eq!(fields(&found), [("n2", "stage")]);
}

#[test]
fn a_stage_or_task_written_with_no_status_is_planned_or_open() {
    let mut log = planned();
    write(
        &mut log,
        &json!([{"id": "t", "kind": "task", "stage": "design", "title": "T"}]),
        &context(&main_agent(), 2),
    );
    let stage = log.find(&crate::common::handle("design")).unwrap();
    assert!(matches!(&stage.current().body, Body::Stage(s) if s.status == Status::Planned));
    let task = log.find(&crate::common::handle("t")).unwrap();
    assert!(matches!(&task.current().body, Body::Task(t) if t.state == TaskState::Open));
}

#[test]
fn a_stage_that_has_never_been_active_names_no_agent() {
    let log = planned();
    assert_eq!(
        log.find(&crate::common::handle("design")).unwrap().owner(),
        None
    );
}

#[test]
fn a_note_with_raw_html_is_refused_and_one_without_a_folded_flag_starts_open() {
    let found = faults(&json!([
        {"id": "n", "kind": "note", "stage": "s", "markdown": "a <b>bold</b> word"}
    ]));
    assert_eq!(fields(&found), [("n", "markdown")]);
    let read = read_batch(&json!([
        {"id": "n", "kind": "note", "stage": "s", "markdown": "| a | b |\n|---|---|\n| 1 | 2 |\n\n```\ncode\n```"}
    ]))
    .unwrap();
    let yardlog::input::Request::Note(note) = &read[0].request else {
        panic!()
    };
    assert!(!note.folded);
}

#[test]
fn a_question_needs_two_options_with_distinct_labels() {
    let one = faults(&json!([{
        "id": "q", "kind": "question", "stage": "s", "prompt": "?", "header": "H",
        "options": [{"label": "A"}]
    }]));
    assert_eq!(fields(&one), [("q", "options")]);
    let repeated = faults(&json!([{
        "id": "q", "kind": "question", "stage": "s", "prompt": "?", "header": "H",
        "options": [{"label": "A"}, {"label": "A"}]
    }]));
    assert_eq!(fields(&repeated), [("q", "options")]);
}

#[test]
fn a_question_that_takes_one_option_recommends_at_most_one() {
    let options = json!([
        {"label": "A", "recommended": true},
        {"label": "B", "recommended": true}
    ]);
    let single = faults(&json!([{
        "id": "q", "kind": "question", "stage": "s", "prompt": "?", "header": "H",
        "options": options
    }]));
    assert_eq!(fields(&single), [("q", "options")]);
    let multiple = json!([{
        "id": "q", "kind": "question", "stage": "s", "prompt": "?", "header": "H",
        "options": options, "multiple": true, "ownWords": true
    }]);
    assert!(read_batch(&multiple).is_ok());
}

#[test]
fn a_decision_written_naming_a_question_records_the_question_s_key() {
    let mut log = planned();
    write(
        &mut log,
        &json!([
            question("q1"),
            {"id": "d1", "kind": "decision", "stage": "design", "title": "D", "consequence": "C", "question": "q1"}
        ]),
        &context(&main_agent(), 2),
    );
    let question = log.find(&crate::common::handle("q1")).unwrap().key;
    let decision = log.find(&crate::common::handle("d1")).unwrap();
    assert!(matches!(&decision.current().body, Body::Decision(d) if d.question == Some(question)));
    assert_eq!(decision.number, Some(1));
}

#[test]
fn a_decision_naming_a_question_yard_log_does_not_hold_is_refused() {
    let mut log = planned();
    let refusal = crate::common::try_write(
        &mut log,
        &json!([{"id": "d1", "kind": "decision", "stage": "design", "title": "D", "consequence": "C", "question": "intro"}]),
        &context(&main_agent(), 2),
    )
    .unwrap_err();
    let yardlog::write::Refusal::Faults(found) = refusal else {
        panic!("{refusal:?}")
    };
    assert_eq!(fields(&found), [("d1", "question")]);
}

#[test]
fn a_new_decision_s_number_follows_the_highest_and_is_kept_through_its_versions() {
    let mut log = planned();
    let decision = |id: &str, title: &str| json!({"id": id, "kind": "decision", "stage": "design", "title": title, "consequence": "C"});
    write(
        &mut log,
        &json!([decision("d1", "One"), decision("d2", "Two")]),
        &context(&main_agent(), 2),
    );
    write(
        &mut log,
        &json!([decision("d1", "One again")]),
        &context(&main_agent(), 3),
    );
    let number = |id| log.find(&crate::common::handle(id)).unwrap().number;
    assert_eq!((number("d1"), number("d2")), (Some(1), Some(2)));
}

#[test]
fn a_loft_block_s_view_decides_which_of_base_decision_and_reasons_it_takes() {
    let found = faults(&json!([
        {"id": "l1", "kind": "loft", "stage": "s", "view": "decision"},
        {"id": "l2", "kind": "loft", "stage": "s", "view": "publish", "base": "develop"},
        {"id": "l3", "kind": "loft", "stage": "s", "view": "trace",
         "reasons": [{"rule": "LQ-005", "key": "01a1646f-546d-7042-befd-ea4a4433bbd3", "reason": "R"}]}
    ]));
    assert_eq!(
        fields(&found),
        [("l1", "decision"), ("l2", "base"), ("l3", "reasons")]
    );
}

#[test]
fn a_batch_that_gives_a_report_a_key_a_time_or_an_answer_is_refused() {
    let found = faults(&json!([
        {"id": "l", "kind": "loft", "stage": "s", "view": "trace", "report": {}},
        {"id": "n", "kind": "note", "stage": "s", "markdown": "N", "key": "01a1646f-546d-7042-befd-ea4a4433bbd3"},
        {"id": "t", "kind": "task", "stage": "s", "title": "T", "startedAt": "2026-10-22T12:00:00Z"},
        {"id": "q", "kind": "question", "stage": "s", "prompt": "?", "header": "H",
         "options": [{"label": "A"}, {"label": "B"}], "answer": "A"},
        {"id": "a", "kind": "approval", "stage": "s", "title": "A", "markdown": "M", "outcome": "approved"}
    ]));
    assert_eq!(
        fields(&found),
        [
            ("l", "report"),
            ("n", "key"),
            ("t", "startedAt"),
            ("q", "answer"),
            ("a", "outcome")
        ]
    );
    assert!(found[3].message.contains("only the person"), "{found:?}");
}

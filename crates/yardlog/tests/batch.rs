//! A batch of versions, `yardlog-batch/1`: each version as the JSON form of its record in its
//! stage's file, beside the keys that place it and its block's kind and id.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use yardlog::YardLog;
use yardlog::batch::{FORMAT, batch, record};
use yardlog::log::{Block, StageFile};

use crate::common::{answer, context, main_agent, person, planned, question, recorded, write};

/// Yard Log with a block of each kind, a decision, a flag and a `loft` snapshot among them.
fn full() -> YardLog {
    let mut log = planned();
    write(
        &mut log,
        &json!([
            question("q1"),
            {"id": "d1", "kind": "decision", "stage": "design", "title": "D", "consequence": "C", "question": "q1"},
            {"id": "t1", "kind": "task", "stage": "design", "title": "T", "state": "active"},
            {"id": "p1", "kind": "plan", "stage": "design", "title": "P", "markdown": "1. one\n"},
            {"id": "a1", "kind": "approval", "stage": "design", "title": "Same", "markdown": "Same."},
            {"id": "n2", "kind": "note", "stage": "design", "title": "Same", "markdown": "Same.", "folded": true},
            {"id": "l1", "kind": "loft", "stage": "design", "view": "findings", "note": "N"}
        ]),
        &context(&main_agent(), 2),
    );
    answer(&mut log, "q1", "A", &person(yardlog::Channel::Terminal, 3));
    log
}

/// The record of the last version of the block `id`, read back as JSON.
fn record_of(log: &YardLog, id: &str) -> Value {
    let (file, block) = find(log, id);
    let version = block.versions.last().unwrap();
    let text = record(log.root.key, file.stage.key.stage(), block, version);
    serde_json::from_str(&text).unwrap()
}

fn find<'a>(log: &'a YardLog, id: &str) -> (&'a StageFile, &'a Block) {
    let key = log.find(&yardlog::Handle::parse(id).unwrap()).unwrap().key;
    log.block(key).unwrap()
}

#[test]
fn a_record_names_its_version_its_yard_log_stage_and_block_and_the_block_s_kind_and_id() {
    let log = full();
    let (file, block) = find(&log, "q1");
    let found = record_of(&log, "q1");
    assert_eq!(found["record"], "version");
    assert_eq!(found["yardLog"], log.root.key.to_string());
    assert_eq!(found["stage"], file.stage.key.to_string());
    assert_eq!(found["block"], block.key.to_string());
    assert_eq!(found["kind"], "question");
    assert_eq!(found["id"], "q1");
    assert_eq!(found["version"]["key"], block.versions[0].key.to_string());
    let mut members: Vec<&String> = found.as_object().unwrap().keys().collect();
    members.sort();
    assert_eq!(
        members,
        [
            "block", "id", "kind", "record", "stage", "version", "yardLog"
        ]
    );
}

#[test]
fn each_kind_of_block_gives_a_record_of_its_kind() {
    let log = full();
    for (id, kind) in [
        ("design", "stage"),
        ("intro", "note"),
        ("q1", "question"),
        ("d1", "decision"),
        ("t1", "task"),
        ("p1", "plan"),
        ("a1", "approval"),
        ("l1", "loft"),
    ] {
        assert_eq!(record_of(&log, id)["kind"], kind, "{id}");
    }
}

#[test]
fn a_stage_s_version_names_the_stage_as_its_block_and_the_stage_s_id() {
    let log = full();
    let found = record_of(&log, "design");
    assert_eq!(found["kind"], "stage");
    assert_eq!(found["block"], found["stage"]);
    assert_eq!(found["id"], "design");
    assert_eq!(found["version"]["title"], "Design");
}

#[test]
fn a_note_and_an_approval_with_the_same_fields_differ_only_in_kind_and_placing() {
    let log = full();
    let (note, approval) = (record_of(&log, "n2"), record_of(&log, "a1"));
    assert_eq!(note["kind"], "note");
    assert_eq!(approval["kind"], "approval");
    for field in ["title", "markdown"] {
        assert_eq!(
            note["version"][field], approval["version"][field],
            "{field}"
        );
    }
}

#[test]
fn a_record_holds_no_decision_number() {
    let log = full();
    assert_eq!(find(&log, "d1").1.number, Some(1));
    let found = record_of(&log, "d1");
    assert!(!found.to_string().contains("\"number\""), "{found}");
}

#[test]
fn the_version_keeps_the_fields_of_its_record_in_the_stage_s_file_in_their_order() {
    let log = full();
    let (file, block) = find(&log, "n2");
    let text = record(
        log.root.key,
        file.stage.key.stage(),
        block,
        &block.versions[0],
    );
    let version = &text[text.find("\"version\":").unwrap()..];
    let order: Vec<usize> = [
        "\"key\"",
        "\"at\"",
        "\"revision\"",
        "\"by\"",
        "\"title\"",
        "\"markdown\"",
        "\"folded\"",
    ]
    .iter()
    .map(|field| version.find(field).unwrap_or_else(|| panic!("{field}")))
    .collect();
    assert!(order.is_sorted(), "{version}");
}

#[test]
fn texts_are_strings_integers_numbers_and_a_flag_the_boolean_true() {
    let log = full();
    let found = record_of(&log, "n2");
    let version = &found["version"];
    assert!(version["at"].is_string());
    assert!(version["revision"].is_u64());
    assert_eq!(version["folded"], Value::Bool(true));
    assert_eq!(
        version["by"],
        json!({"host": "claude-code", "session": "session-a"})
    );
    let question = record_of(&log, "q1");
    assert_eq!(question["version"]["options"][0], json!({"label": "A"}));
}

#[test]
fn a_loft_block_s_report_is_nested_json_and_not_text() {
    let log = full();
    let found = record_of(&log, "l1");
    assert_eq!(found["version"]["report"], json!({"nodes": []}));
    assert_eq!(found["version"]["recorded"]["branch"], "feat/x");
    let (_, datum) = recorded();
    assert_eq!(found["version"]["report"], Value::from(&datum));
}

#[test]
fn writing_records_changes_no_byte_of_yard_log_s_files() {
    let log = full();
    let before: BTreeMap<String, String> = log.files().into_iter().collect();
    for (stage, block, version) in log.versions() {
        let _ = record(log.root.key, stage, block, version);
    }
    let after: BTreeMap<String, String> = log.files().into_iter().collect();
    assert_eq!(before, after);
}

#[test]
fn a_batch_names_its_format_and_holds_its_records_in_order() {
    let log = full();
    let records = [
        record_of(&log, "intro").to_string(),
        record_of(&log, "q1").to_string(),
    ];
    let found: Value = serde_json::from_str(&batch(&records)).unwrap();
    assert_eq!(found["format"], FORMAT);
    assert_eq!(FORMAT, "yardlog-batch/1");
    assert_eq!(found["records"][0]["id"], "intro");
    assert_eq!(found["records"][1]["id"], "q1");
    assert_eq!(found.as_object().unwrap().len(), 2);
}

#[test]
fn a_record_carries_no_answer_person_submission_root_or_branch_outside_its_version() {
    let mut log = full();
    write(
        &mut log,
        &json!([question("q2")]),
        &yardlog::write::Context {
            send: true,
            ..context(&main_agent(), 4)
        },
    );
    for id in ["q1", "q2", "intro"] {
        let text = record_of(&log, id).to_string();
        for absent in [
            "Ada",
            "ada@example.com",
            "chosen",
            "submission",
            "feat/x",
            "develop",
        ] {
            assert!(!text.contains(absent), "{id} holds {absent}: {text}");
        }
    }
    let mut snapshot = record_of(&log, "l1");
    assert_eq!(snapshot["version"]["recorded"]["branch"], "feat/x");
    snapshot["version"]["recorded"]["branch"] = Value::Null;
    assert!(!snapshot.to_string().contains("feat/x"), "{snapshot}");
}

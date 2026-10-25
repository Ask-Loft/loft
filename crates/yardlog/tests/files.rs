//! Yard Log as files: a root and one file for each stage, named by its key, in one canonical
//! layout.

use std::collections::BTreeMap;

use serde_json::json;
use yardlog::codec::{ROOT, read_format, stage_file};
use yardlog::{Format, YardLog};

use crate::common::{answer, context, handle, main_agent, person, planned, question, write};

fn files(log: &YardLog) -> BTreeMap<String, String> {
    log.files().into_iter().collect()
}

fn read(files: &BTreeMap<String, String>) -> Result<YardLog, yardlog::codec::FileError> {
    YardLog::read(files[ROOT].as_bytes(), |name| {
        files.get(name).map(|text| text.clone().into_bytes())
    })
}

/// Yard Log with a block of each kind, answered, sent and versioned.
fn full() -> YardLog {
    let mut log = planned();
    write(
        &mut log,
        &json!([
            {"id": "design", "kind": "stage", "title": "Design", "goal": "A *goal*.\n\nTwo lines.", "status": "active"},
            {"id": "build", "kind": "stage", "title": "Build"},
            question("q1"),
            {"id": "d1", "kind": "decision", "stage": "design", "title": "D", "consequence": "C", "question": "q1"},
            {"id": "t1", "kind": "task", "stage": "build", "title": "T", "detail": "true", "state": "active"},
            {"id": "t2", "kind": "task", "stage": "build", "title": "F", "state": "open", "type": "fix"},
            {"id": "p1", "kind": "plan", "stage": "build", "title": "P", "markdown": "1. one\n2. two\n", "path": "plan.md", "planMode": true},
            {"id": "a1", "kind": "approval", "stage": "build", "title": "A", "markdown": ""},
            {"id": "n2", "kind": "note", "stage": "build", "markdown": "12", "tone": "warning", "folded": true, "title": "null"},
            {"id": "l1", "kind": "loft", "stage": "build", "view": "findings", "note": "N",
             "reasons": [{"rule": "LQ-005", "key": "01a1646f-546d-7042-befd-ea4a4433bbd3", "reason": "R"}]}
        ]),
        &yardlog::write::Context {
            send: true,
            ..context(&crate::common::subagent(), 2)
        },
    );
    answer(&mut log, "q1", "A", &person(yardlog::Channel::Terminal, 3));
    log
}

#[test]
fn yard_log_reads_back_from_its_files_as_it_was_written() {
    let log = full();
    assert_eq!(read(&files(&log)).unwrap(), log);
}

#[test]
fn the_root_names_the_format_the_key_the_branch_the_revision_and_the_stages_by_key() {
    let log = full();
    let root = &files(&log)[ROOT];
    assert!(root.starts_with("format: yardlog/1\nkey: "), "{root}");
    assert!(
        root.contains("\nbranch: \"feat/x\"\n") || root.contains("\nbranch: feat/x\n"),
        "{root}"
    );
    assert!(root.contains("\nrevision: 3\n"), "{root}");
    let design = log.root.stages[0].key;
    assert!(
        root.contains(&format!("stages:\n  - key: {design}\n    id: design\n")),
        "{root}"
    );
    assert_eq!(read_format(root.as_bytes()).unwrap(), Format::CURRENT);
}

#[test]
fn the_root_records_the_base_branch_yard_log_is_merged_into_and_reads_it_back() {
    let log = full();
    let root = &files(&log)[ROOT];
    assert!(root.contains("\nbase: \"develop\"\n"), "{root}");
    assert_eq!(read(&files(&log)).unwrap().root.base, "develop");
    let without = root.replace("\nbase: \"develop\"\n", "\n");
    let mut changed = files(&log);
    changed.insert(ROOT.to_owned(), without);
    assert!(
        read(&changed).is_err(),
        "a root without its base branch reads"
    );
}

#[test]
fn the_root_holds_the_act_of_each_change_in_the_order_it_was_made() {
    let log = full();
    let revisions: Vec<u64> = log.root.acts.iter().map(|act| act.revision).collect();
    assert_eq!(revisions, [1, 2, 3]);
    let root = &files(&log)[ROOT];
    assert!(root.contains("\nacts:\n  - at: "), "{root}");
    assert_eq!(read(&files(&log)).unwrap().root.acts, log.root.acts);
}

#[test]
fn each_stage_s_file_is_named_by_its_key_and_holds_its_blocks_in_order() {
    let log = full();
    let files = files(&log);
    let build = log.root.stages[1].key;
    let text = &files[&stage_file(build)];
    assert!(
        text.starts_with(&format!("key: {build}\nid: build\nversions:\n")),
        "{text}"
    );
    let order: Vec<usize> = ["id: t1", "id: p1", "id: a1", "id: n2", "id: l1"]
        .iter()
        .map(|id| text.find(id).unwrap())
        .collect();
    assert!(order.is_sorted(), "{text}");
}

#[test]
fn a_regular_task_is_kept_without_a_type_and_a_fix_with_its_type() {
    let log = full();
    let files = files(&log);
    let text = &files[&stage_file(log.root.stages[1].key)];
    let regular = text.find("id: t1\n").unwrap();
    let fix = text.find("id: t2\n").unwrap();
    let after = text.find("id: p1\n").unwrap();
    assert!(!text[regular..fix].contains("type:"), "{text}");
    assert!(text[fix..after].contains("type: fix"), "{text}");
}

#[test]
fn writing_yard_log_that_has_not_changed_gives_the_same_bytes() {
    let log = full();
    let first = files(&log);
    assert_eq!(files(&read(&first).unwrap()), first);
}

#[test]
fn a_change_to_one_block_rewrites_only_its_stage_s_file_and_the_root() {
    let mut log = full();
    let before = files(&log);
    write(
        &mut log,
        &json!([{"id": "intro", "kind": "note", "stage": "design", "markdown": "Changed."}]),
        &context(&main_agent(), 4),
    );
    let after = files(&log);
    let mut changed: Vec<String> = after
        .keys()
        .filter(|name| before.get(*name) != after.get(*name))
        .cloned()
        .collect();
    changed.sort();
    let mut expected = vec![stage_file(log.root.stages[0].key), ROOT.to_owned()];
    expected.sort();
    assert_eq!(changed, expected);
}

#[test]
fn the_files_hold_no_comments_anchors_aliases_tags_or_flow_collections() {
    // The strict reader of `spec/` refuses each of these at its line and column.
    for (name, text) in files(&full()) {
        let read = loft_common::yaml::read(text.as_bytes());
        assert!(matches!(read, Ok(Some(_))), "{name}: {read:?}");
    }
}

#[test]
fn a_file_that_does_not_read_is_named_with_the_line_at_fault() {
    let log = full();
    let mut found = files(&log);
    let design = stage_file(log.root.stages[0].key);
    let broken = found[&design].replacen("versions:", "versions: [", 1);
    found.insert(design.clone(), broken);
    let error = read(&found).unwrap_err();
    assert_eq!(error.file, design);
    assert_eq!(error.error.position.map(|p| p.line), Some(3));
}

#[test]
fn a_reference_to_a_key_yard_log_does_not_hold_is_refused_at_its_line() {
    let log = full();
    let mut found = files(&log);
    let question = log.find(&handle("q1")).unwrap().key.to_string();
    let design = stage_file(log.root.stages[0].key);
    let text = found[&design].clone();
    let line = text
        .lines()
        .position(|line| line.trim() == format!("question: {question}"))
        .unwrap()
        + 1;
    let absent = "01a1646f-0000-7000-8000-000000000000";
    found.insert(
        design.clone(),
        text.replace(
            &format!("question: {question}"),
            &format!("question: {absent}"),
        ),
    );
    let error = read(&found).unwrap_err();
    assert_eq!(
        (error.file.as_str(), error.error.position.map(|p| p.line)),
        (design.as_str(), Some(line))
    );
    assert!(error.error.message.contains(absent), "{error}");
}

#[test]
fn a_root_of_a_newer_format_is_told_apart() {
    let log = full();
    let root = files(&log)[ROOT].replacen("yardlog/1", "yardlog/2", 1);
    assert_eq!(read_format(root.as_bytes()).unwrap(), Format::new(2));
    assert!(!Format::new(2).is_known());
    assert!(yardlog::codec::read_root(root.as_bytes()).is_err());
}

#[test]
fn upgrading_older_yard_log_records_the_person_as_the_actor_of_one_act() {
    let mut log = crate::common::planned();
    log.root.format = yardlog::Format::new(0);
    let before = log.root.revision;
    let person = yardlog::Actor::Person {
        name: "Ada".to_owned(),
        email: "ada@example.com".to_owned(),
    };
    let from = log.upgrade(crate::common::at(9), person.clone());
    assert_eq!(from, Some(yardlog::Format::new(0)));
    assert_eq!(log.root.format, yardlog::Format::CURRENT);
    assert_eq!(log.root.revision, before + 1);
    let act = log.root.acts.last().unwrap();
    assert_eq!((act.revision, &act.actor), (before + 1, &person));
    assert_eq!(log.upgrade(crate::common::at(10), person), None);
    assert_eq!(log.root.revision, before + 1);
}

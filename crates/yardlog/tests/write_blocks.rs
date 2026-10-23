//! Writing a batch of blocks: whole or not at all, every version kept.

use serde_json::json;
use yardlog::act::{Actor, Time};
use yardlog::body::Body;
use yardlog::write::{Context, Refusal};

use crate::common::{
    at, context, handle, main_agent, opened, other_session, planned, question, subagent, try_write,
    write,
};

fn ids(log: &yardlog::YardLog, stage: &str) -> Vec<String> {
    let file = log
        .stages
        .iter()
        .find(|file| file.stage.id == handle(stage))
        .unwrap();
    file.blocks
        .iter()
        .map(|block| block.id.to_string())
        .collect()
}

#[test]
fn the_first_write_opens_yard_log_titled_with_its_branch() {
    let mut log = opened();
    let written = write(
        &mut log,
        &json!([{"id": "design", "kind": "stage", "title": "Design"}]),
        &context(&main_agent(), 5),
    );
    assert_eq!(written.revision, 1);
    assert_eq!(log.root.branch, "feat/x");
    assert_eq!(log.root.title, "feat/x");
    assert_eq!(log.root.opened.revision, 1);
    assert_eq!(log.root.key.key().version(), 7);
}

#[test]
fn a_valid_batch_raises_the_revision_by_one_and_gives_each_block_s_key() {
    let mut log = planned();
    let written = write(
        &mut log,
        &json!([
            {"id": "n1", "kind": "note", "stage": "design", "markdown": "One"},
            {"id": "n2", "kind": "note", "stage": "design", "markdown": "Two"}
        ]),
        &context(&main_agent(), 2),
    );
    assert_eq!(written.revision, 2);
    assert_eq!(log.root.revision, 2);
    let keys: Vec<_> = written
        .blocks
        .iter()
        .map(|(id, key)| (id.to_string(), *key))
        .collect();
    assert_eq!(
        keys[0],
        ("n1".to_owned(), log.find(&handle("n1")).unwrap().key)
    );
    assert_eq!(keys[1].0, "n2");
}

#[test]
fn a_batch_with_one_invalid_block_changes_nothing() {
    let mut log = planned();
    let before = log.clone();
    let refusal = try_write(
        &mut log,
        &json!([
            {"id": "n1", "kind": "note", "stage": "design", "markdown": "One"},
            {"id": "n2", "kind": "note", "stage": "nowhere", "markdown": "Two"}
        ]),
        &context(&main_agent(), 2),
    )
    .unwrap_err();
    assert!(
        matches!(&refusal, Refusal::Faults(faults) if faults[0].block == "n2" && faults[0].field == "stage")
    );
    assert_eq!(log, before);
}

#[test]
fn a_block_naming_a_stage_the_batch_writes_only_after_it_is_refused_not_a_crash() {
    let mut log = planned();
    let before = log.clone();
    let refusal = try_write(
        &mut log,
        &json!([
            {"id": "n1", "kind": "note", "stage": "later", "markdown": "Early"},
            {"id": "later", "kind": "stage", "title": "Later"}
        ]),
        &context(&main_agent(), 2),
    )
    .unwrap_err();
    assert!(
        matches!(&refusal, Refusal::Faults(faults) if faults[0].block == "n1" && faults[0].field == "stage"),
        "{refusal:?}"
    );
    assert_eq!(log, before);
}

#[test]
fn a_new_block_goes_at_the_end_of_its_stage_or_after_the_block_it_names() {
    let mut log = planned();
    write(
        &mut log,
        &json!([
            {"id": "last", "kind": "note", "stage": "design", "markdown": "L"},
            {"id": "second", "kind": "note", "stage": "design", "markdown": "S", "after": "intro"},
            {"id": "build", "kind": "stage", "title": "Build"},
            {"id": "plan", "kind": "stage", "title": "Plan", "after": "design"}
        ]),
        &context(&main_agent(), 2),
    );
    assert_eq!(ids(&log, "design"), ["intro", "second", "last"]);
    let stages: Vec<String> = log
        .root
        .stages
        .iter()
        .map(|entry| entry.id.to_string())
        .collect();
    assert_eq!(stages, ["design", "plan", "build"]);
}

#[test]
fn a_block_written_again_gains_a_version_and_keeps_its_key_and_earlier_versions() {
    let mut log = planned();
    let key = log.find(&handle("intro")).unwrap().key;
    write(
        &mut log,
        &json!([{"id": "intro", "kind": "note", "stage": "design", "markdown": "Hello again."}]),
        &context(&main_agent(), 2),
    );
    let block = log.find(&handle("intro")).unwrap();
    assert_eq!(block.key, key);
    assert_eq!(block.versions.len(), 2);
    assert!(matches!(&block.current().body, Body::Note(n) if n.markdown == "Hello again."));
    assert_eq!(block.number_of(block.current()), 2);
    assert_ne!(block.versions[0].key, block.versions[1].key);
}

#[test]
fn a_block_written_again_as_another_kind_or_in_another_stage_is_refused() {
    let mut log = planned();
    write(
        &mut log,
        &json!([{"id": "build", "kind": "stage", "title": "Build"}]),
        &context(&main_agent(), 2),
    );
    for batch in [
        json!([{"id": "intro", "kind": "task", "stage": "design", "title": "T"}]),
        json!([{"id": "intro", "kind": "note", "stage": "build", "markdown": "M"}]),
    ] {
        let refusal = try_write(&mut log, &batch, &context(&main_agent(), 3)).unwrap_err();
        assert!(matches!(refusal, Refusal::Faults(_)), "{refusal:?}");
    }
}

#[test]
fn each_version_records_the_act_of_its_write() {
    let mut log = planned();
    write(
        &mut log,
        &json!([{"id": "intro", "kind": "note", "stage": "design", "markdown": "Again."}]),
        &context(&subagent(), 7),
    );
    let version = log.find(&handle("intro")).unwrap().current().clone();
    assert_eq!(version.act.revision, 2);
    assert_eq!(version.act.at, at(7));
    assert_eq!(version.act.at.to_string(), "2026-10-22T12:00:07Z");
    assert_eq!(version.act.actor, Actor::Agent(subagent()));
}

#[test]
fn a_stage_records_when_it_was_listed_started_and_ended() {
    let mut log = planned();
    let stage = |status: &str| json!([{"id": "design", "kind": "stage", "title": "Design", "status": status}]);
    write(&mut log, &stage("active"), &context(&main_agent(), 10));
    write(&mut log, &stage("active"), &context(&main_agent(), 20));
    write(&mut log, &stage("done"), &context(&main_agent(), 30));
    write(&mut log, &stage("done"), &context(&main_agent(), 40));
    let times = log.find(&handle("design")).unwrap().times();
    let expected = |s| Some::<Time>(at(s));
    assert_eq!(
        (times.listed, times.started, times.ended),
        (expected(1), expected(10), expected(30))
    );
    write(&mut log, &stage("active"), &context(&main_agent(), 50));
    let times = log.find(&handle("design")).unwrap().times();
    assert_eq!((times.started, times.ended), (expected(10), None));
}

#[test]
fn a_block_an_open_submission_holds_is_refused_until_the_submission_is_withdrawn() {
    let mut log = planned();
    let sent = write(
        &mut log,
        &json!([question("q1")]),
        &Context {
            send: true,
            ..context(&main_agent(), 2)
        },
    );
    let submission = sent.submission.unwrap();
    let rewrite = json!([question("q1")]);
    let refusal = try_write(&mut log, &rewrite, &context(&main_agent(), 3)).unwrap_err();
    assert_eq!(
        refusal,
        Refusal::Held {
            block: handle("q1"),
            submission
        }
    );
    log.withdraw(submission, main_agent(), at(4)).unwrap();
    write(&mut log, &rewrite, &context(&main_agent(), 5));
}

#[test]
fn a_batch_based_on_an_older_revision_is_refused_for_the_blocks_changed_since() {
    let mut log = planned();
    write(
        &mut log,
        &json!([{"id": "intro", "kind": "note", "stage": "design", "markdown": "Changed."}]),
        &context(&other_session(), 2),
    );
    let stale = Context {
        base: Some(1),
        ..context(&main_agent(), 3)
    };
    let refusal = try_write(
        &mut log,
        &json!([{"id": "intro", "kind": "note", "stage": "design", "markdown": "Mine."}]),
        &stale,
    )
    .unwrap_err();
    let key = log.find(&handle("intro")).unwrap().key;
    assert_eq!(
        refusal,
        Refusal::Stale {
            base: 1,
            revision: 2,
            blocks: vec![key]
        }
    );
    write(
        &mut log,
        &json!([{"id": "other", "kind": "note", "stage": "design", "markdown": "New."}]),
        &stale,
    );
}

#[test]
fn an_agent_keeps_at_most_one_stage_active_and_a_stage_passes_to_who_makes_it_active() {
    let mut log = planned();
    let active = |id: &str| json!({"id": id, "kind": "stage", "title": id, "status": "active"});
    write(
        &mut log,
        &json!([active("design")]),
        &context(&main_agent(), 2),
    );
    write(
        &mut log,
        &json!([active("build")]),
        &context(&subagent(), 3),
    );
    let refusal = try_write(
        &mut log,
        &json!([active("review")]),
        &context(&main_agent(), 4),
    )
    .unwrap_err();
    assert!(
        matches!(&refusal, Refusal::Faults(f) if f[0].block == "review"),
        "{refusal:?}"
    );
    write(
        &mut log,
        &json!([active("design")]),
        &context(&other_session(), 5),
    );
    let owner = log.find(&handle("design")).unwrap().owner().cloned();
    assert_eq!(owner, Some(other_session()));
}

#[test]
fn a_sent_batch_opens_a_submission_of_its_questions_approvals_and_plans_in_order() {
    let mut log = planned();
    let written = write(
        &mut log,
        &json!([
            {"id": "a1", "kind": "approval", "stage": "design", "title": "A", "markdown": "OK?"},
            {"id": "n1", "kind": "note", "stage": "design", "markdown": "N"},
            question("q1")
        ]),
        &Context {
            send: true,
            ..context(&subagent(), 2)
        },
    );
    let submission = log.submission(written.submission.unwrap()).unwrap();
    let expected = [handle("a1"), handle("q1")].map(|id| log.find(&id).unwrap().key);
    assert_eq!(submission.blocks, expected);
    assert_eq!(submission.sender(), &Actor::Agent(subagent()));
    assert_eq!(submission.sending().at, at(2));
    assert_eq!(submission.state(&log), yardlog::query::State::Open);
}

#[test]
fn a_sent_batch_with_nothing_to_ask_is_refused_and_an_unsent_one_opens_no_submission() {
    let mut log = planned();
    let note = json!([{"id": "n1", "kind": "note", "stage": "design", "markdown": "N"}]);
    let refusal = try_write(
        &mut log,
        &note,
        &Context {
            send: true,
            ..context(&main_agent(), 2)
        },
    )
    .unwrap_err();
    assert_eq!(refusal, Refusal::NothingToSend);
    let written = write(
        &mut log,
        &json!([question("q1")]),
        &context(&main_agent(), 3),
    );
    assert_eq!(written.submission, None);
    assert_eq!(log.root.submissions, []);
}

#[test]
fn a_loft_block_records_its_report_and_is_never_written_again() {
    let mut log = planned();
    let block = json!([{"id": "trace", "kind": "loft", "stage": "design", "view": "trace", "note": "Coverage."}]);
    write(&mut log, &block.clone(), &context(&main_agent(), 2));
    let found = log.find(&handle("trace")).unwrap();
    assert!(matches!(&found.current().body, Body::Loft(loft) if loft.recorded.loft == "2.2.1"));
    let refusal = try_write(&mut log, &block, &context(&main_agent(), 3)).unwrap_err();
    assert!(matches!(refusal, Refusal::Faults(_)), "{refusal:?}");
}

#[test]
fn a_report_that_cannot_be_computed_refuses_the_batch() {
    let mut log = planned();
    let before = log.clone();
    let inputs = crate::common::inputs(&json!([
        {"id": "n", "kind": "note", "stage": "design", "markdown": "N"},
        {"id": "trace", "kind": "loft", "stage": "design", "view": "trace"}
    ]));
    let reports = [(handle("trace"), Err("no spec/".to_owned()))].into();
    let refusal = log
        .write(&inputs, &context(&main_agent(), 2), &reports)
        .unwrap_err();
    assert!(
        matches!(&refusal, Refusal::Report { block, view: "trace", .. } if *block == handle("trace"))
    );
    assert_eq!(log, before);
}

#[test]
fn closed_yard_log_refuses_every_batch() {
    let mut log = planned();
    log.close(main_agent(), at(2)).unwrap();
    let refusal = try_write(
        &mut log,
        &json!([question("q1")]),
        &context(&main_agent(), 3),
    )
    .unwrap_err();
    assert_eq!(refusal, Refusal::Closed);
}

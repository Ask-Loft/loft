//! Two copies of Yard Log merged one file at a time, then settled.

use std::collections::BTreeMap;

use serde_json::json;
use yardlog::YardLog;
use yardlog::act::{Actor, Channel};
use yardlog::codec::ROOT;
use yardlog::log::Arrival;
use yardlog::merge::merge_file;
use yardlog::query::State;
use yardlog::write::Context;

use crate::common::{
    answer, at, context, handle, main_agent, opened, other_session, person, planned, question,
    write,
};

fn files(log: &YardLog) -> BTreeMap<String, String> {
    log.files().into_iter().collect()
}

/// `ours` and `theirs` merged as git merges them: each file both copies hold through the
/// driver, and a file one copy adds as it is.
fn merged(base: Option<&YardLog>, ours: &YardLog, theirs: &YardLog) -> YardLog {
    let base = base.map(files).unwrap_or_default();
    let (ours, theirs) = (files(ours), files(theirs));
    let mut result = BTreeMap::new();
    for name in ours.keys().chain(theirs.keys()) {
        let text = match (ours.get(name), theirs.get(name)) {
            (Some(o), Some(t)) => merge_file(
                name,
                base.get(name).map(String::as_bytes),
                o.as_bytes(),
                t.as_bytes(),
            )
            .unwrap_or_else(|(which, error)| panic!("{which}: {error}")),
            (Some(one), None) | (None, Some(one)) => one.clone(),
            (None, None) => unreachable!(),
        };
        result.insert(name.clone(), text);
    }
    YardLog::read(result[ROOT].as_bytes(), |name| {
        result.get(name).map(|t| t.clone().into_bytes())
    })
    .unwrap()
}

fn note(id: &str, markdown: &str) -> serde_json::Value {
    json!([{"id": id, "kind": "note", "stage": "design", "markdown": markdown}])
}

fn ids(log: &YardLog) -> Vec<String> {
    log.blocks().map(|block| block.id.to_string()).collect()
}

#[test]
fn records_of_both_copies_join_by_key_with_every_version_once() {
    let base = planned();
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    write(
        &mut ours,
        &note("intro", "Ours."),
        &context(&main_agent(), 10),
    );
    write(&mut ours, &note("mine", "M"), &context(&main_agent(), 11));
    write(
        &mut theirs,
        &note("intro", "Theirs."),
        &context(&other_session(), 20),
    );
    write(
        &mut theirs,
        &note("yours", "Y"),
        &context(&other_session(), 21),
    );
    let result = merged(Some(&base), &ours, &theirs);
    assert_eq!(ids(&result), ["design", "intro", "mine", "yours"]);
    let intro = result.find(&handle("intro")).unwrap();
    assert_eq!(intro.versions.len(), 3);
    assert!(
        matches!(&intro.current().body, yardlog::body::Body::Note(n) if n.markdown == "Theirs.")
    );
    let arrived: Vec<_> = intro
        .ordered()
        .iter()
        .map(|version| version.arrived)
        .collect();
    assert_eq!(arrived, [None, None, Some(Arrival::Pending)]);
}

#[test]
fn stages_keep_the_base_s_order_followed_by_each_copy_s_new_ones() {
    let base = planned();
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    let stage = |id: &str| json!([{"id": id, "kind": "stage", "title": id, "after": "design"}]);
    write(&mut ours, &stage("ours"), &context(&main_agent(), 10));
    write(
        &mut theirs,
        &stage("theirs"),
        &context(&other_session(), 10),
    );
    let result = merged(Some(&base), &ours, &theirs);
    let order: Vec<String> = result
        .root
        .stages
        .iter()
        .map(|e| e.id.to_string())
        .collect();
    assert_eq!(order, ["design", "ours", "theirs"]);
}

#[test]
fn a_submission_only_moves_forward_and_the_earlier_answer_counts() {
    let mut base = planned();
    let key = write(
        &mut base,
        &json!([question("q1")]),
        &Context {
            send: true,
            ..context(&main_agent(), 2)
        },
    )
    .submission
    .unwrap();
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    answer(&mut ours, "q1", "B", &person(Channel::Page, 30));
    answer(&mut theirs, "q1", "A", &person(Channel::Terminal, 20));
    let result = merged(Some(&base), &ours, &theirs);
    assert_eq!(
        result.submission(key).unwrap().state(&result),
        State::Answered
    );
    let block = result.find(&handle("q1")).unwrap();
    assert_eq!(block.responses.len(), 2);
    assert_eq!(block.counted().unwrap().channel, Channel::Terminal);
    let ours_answer = &ours.find(&handle("q1")).unwrap().responses[0];
    assert!(
        block.responses.contains(ours_answer),
        "neither answer is rewritten"
    );
    let mut withdrawn = base.clone();
    withdrawn.withdraw(key, main_agent(), at(5)).unwrap();
    let result = merged(Some(&base), &base, &withdrawn);
    assert_eq!(
        result.submission(key).unwrap().state(&result),
        State::Withdrawn
    );
}

#[test]
fn yard_log_closed_in_either_copy_is_closed_by_the_earlier_close() {
    let base = planned();
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    ours.close(main_agent(), at(40)).unwrap();
    theirs.close(other_session(), at(30)).unwrap();
    let result = merged(Some(&base), &ours, &theirs);
    assert_eq!(result.root.closed.unwrap().at, at(30));
}

#[test]
fn the_result_s_revision_is_one_past_the_higher_and_unsettled_since_this_copy_s() {
    let base = planned();
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    write(&mut ours, &note("a", "A"), &context(&main_agent(), 10));
    for n in 0..3 {
        write(
            &mut theirs,
            &note(&format!("b{n}"), "B"),
            &context(&other_session(), 10 + n),
        );
    }
    let result = merged(Some(&base), &ours, &theirs);
    assert_eq!(result.root.revision, 5);
    assert_eq!(result.root.unsettled_since, Some(2));
    let again = merged(Some(&base), &result, &theirs);
    assert_eq!(again.root.unsettled_since, Some(2));
}

#[test]
fn the_driver_changes_no_id_and_no_number_and_settling_does() {
    let base = planned();
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    let decision = |id: &str| json!({"id": id, "kind": "decision", "stage": "design", "title": id, "consequence": "C"});
    write(
        &mut ours,
        &json!([decision("ours"), {"id": "same", "kind": "note", "stage": "design", "markdown": "O"}]),
        &context(&main_agent(), 20),
    );
    write(
        &mut theirs,
        &json!([decision("theirs"), {"id": "same", "kind": "note", "stage": "design", "markdown": "T"}]),
        &context(&other_session(), 10),
    );
    let mut result = merged(Some(&base), &ours, &theirs);
    let number = |log: &YardLog, id: &str| log.find(&handle(id)).unwrap().number;
    assert_eq!(
        (number(&result, "ours"), number(&result, "theirs")),
        (Some(1), Some(1))
    );
    assert_eq!(ids(&result).iter().filter(|id| *id == "same").count(), 2);
    let settling = result
        .settle(at(50), Actor::Agent(main_agent()))
        .unwrap()
        .clone();
    assert_eq!(
        (number(&result, "theirs"), number(&result, "ours")),
        (Some(1), Some(2))
    );
    let ours_note = ours.find(&handle("same")).unwrap().key;
    assert_eq!(result.block(ours_note).unwrap().1.id, handle("same-2"));
    assert_eq!(
        settling.renamed,
        [(ours_note, handle("same"), handle("same-2"))]
    );
    assert_eq!(settling.renumbered.len(), 1);
    assert_eq!(settling.act.revision, result.root.revision);
}

#[test]
fn of_two_records_written_at_one_time_the_greater_key_is_the_later() {
    let base = planned();
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    write(&mut ours, &note("same", "O"), &context(&main_agent(), 10));
    write(
        &mut theirs,
        &note("same", "T"),
        &context(&other_session(), 10),
    );
    let mut result = merged(Some(&base), &ours, &theirs);
    result.settle(at(50), Actor::Agent(main_agent()));
    let keys = [&ours, &theirs].map(|log| log.find(&handle("same")).unwrap().key);
    let later = keys.iter().max().unwrap();
    assert_eq!(result.block(*later).unwrap().1.id, handle("same-2"));
}

#[test]
fn settling_marks_what_the_merge_brought_and_settles_only_once() {
    let base = planned();
    let (ours, mut theirs) = (base.clone(), base.clone());
    write(
        &mut theirs,
        &note("yours", "Y"),
        &context(&other_session(), 10),
    );
    let mut result = merged(Some(&base), &ours, &theirs);
    assert_eq!(
        result.find(&handle("yours")).unwrap().versions[0].arrived,
        Some(Arrival::Pending)
    );
    let revision = result
        .settle(at(50), Actor::Agent(main_agent()))
        .unwrap()
        .act
        .revision;
    let yours = result.find(&handle("yours")).unwrap();
    assert_eq!(yours.versions[0].arrived, Some(Arrival::At(revision)));
    assert_eq!(yours.reached(), revision);
    assert_eq!(result.find(&handle("intro")).unwrap().reached(), 1);
    assert_eq!(result.root.unsettled_since, None);
    assert!(result.settle(at(60), Actor::Agent(main_agent())).is_none());
}

#[test]
fn yard_logs_opened_apart_join_under_the_earlier_key() {
    let mut early = opened();
    write(
        &mut early,
        &json!([{"id": "design", "kind": "stage", "title": "D"}]),
        &context(&main_agent(), 1),
    );
    let mut late = YardLog::open("feat/x", "develop", at(5), other_session());
    write(
        &mut late,
        &json!([{"id": "build", "kind": "stage", "title": "B"}]),
        &context(&other_session(), 6),
    );
    let result = merged(None, &late, &early);
    assert_eq!(result.root.key, early.root.key);
    assert_eq!(result.root.merged, [late.root.key]);
    assert_eq!(ids(&result), ["build", "design"]);
}

#[test]
fn a_copy_that_does_not_read_as_yardlog_fails_the_merge_naming_it() {
    let log = planned();
    let root = &files(&log)[ROOT];
    let (which, error) =
        merge_file(ROOT, None, root.as_bytes(), b"format: yardlog/1\nkey: [\n").unwrap_err();
    assert_eq!(which, "theirs");
    assert_eq!(error.file, ROOT);
}

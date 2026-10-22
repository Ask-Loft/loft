//! The person's answers and outcomes, and the changes of a submission's state.

use serde_json::json;
use yardlog::SubmissionKey;
use yardlog::act::{Actor, Channel};
use yardlog::body::Verdict;
use yardlog::log::{Given, Happened};
use yardlog::query::State;
use yardlog::respond::{Refused, Reply};
use yardlog::write::Context;

use crate::common::{
    answer, at, context, handle, main_agent, other_session, person, planned, question, subagent,
    write,
};

fn sent(log: &mut yardlog::YardLog, batch: &serde_json::Value, seconds: u64) -> SubmissionKey {
    write(
        log,
        batch,
        &Context {
            send: true,
            ..context(&main_agent(), seconds)
        },
    )
    .submission
    .unwrap()
}

fn outcome(log: &yardlog::YardLog, id: &str, verdict: Verdict, reason: Option<&str>) -> Reply {
    Reply {
        block: log.find(&handle(id)).unwrap().key,
        given: Given::Outcome {
            verdict,
            reason: reason.map(str::to_owned),
        },
    }
}

#[test]
fn an_answer_records_the_person_its_act_and_its_channel() {
    let mut log = planned();
    sent(&mut log, &json!([question("q1")]), 2);
    let answered = answer(&mut log, "q1", "B", &person(Channel::Terminal, 3));
    assert_eq!(answered.revision, 3);
    let block = log.find(&handle("q1")).unwrap();
    let counted = block.counted().unwrap();
    assert_eq!(counted.channel, Channel::Terminal);
    assert_eq!(
        counted.act.actor,
        Actor::Person {
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned()
        }
    );
    assert!(matches!(&counted.given, Given::Answer { chosen, .. } if chosen == &["B"]));
    assert_eq!(counted.key.key().version(), 7);
}

#[test]
fn a_submission_becomes_answered_once_its_last_open_block_is_answered() {
    let mut log = planned();
    let key = sent(
        &mut log,
        &json!([question("q1"), {"id": "a1", "kind": "approval", "stage": "design", "title": "A", "markdown": "OK?"}]),
        2,
    );
    answer(&mut log, "q1", "A", &person(Channel::Page, 3));
    assert_eq!(log.submission(key).unwrap().state(&log), State::Open);
    let reply = outcome(&log, "a1", Verdict::Approved, None);
    let answered = log.answer(&[reply], &person(Channel::Page, 4)).unwrap();
    assert_eq!(answered.completed, [key]);
    let submission = log.submission(key).unwrap();
    assert_eq!(submission.state(&log), State::Answered);
    assert_eq!(submission.changes.last().unwrap().what, Happened::Answered);
}

#[test]
fn a_later_answer_to_a_block_answered_elsewhere_is_refused_and_the_first_kept() {
    let mut log = planned();
    sent(&mut log, &json!([question("q1"), question("q2")]), 2);
    answer(&mut log, "q1", "A", &person(Channel::Terminal, 3));
    let keys = [handle("q1"), handle("q2")].map(|id| log.find(&id).unwrap().key);
    let replies = keys.map(|block| Reply {
        block,
        given: Given::Answer {
            chosen: vec!["B".to_owned()],
            words: None,
            notes: None,
        },
    });
    let answered = log.answer(&replies, &person(Channel::Page, 4)).unwrap();
    assert_eq!(answered.elsewhere, [(keys[0], Channel::Terminal)]);
    assert_eq!(answered.recorded, [keys[1]]);
    let first = log.find(&handle("q1")).unwrap();
    assert_eq!(first.responses.len(), 1);
    assert!(
        matches!(&first.counted().unwrap().given, Given::Answer { chosen, .. } if chosen == &["A"])
    );
}

#[test]
fn an_answer_must_fit_its_question() {
    let mut log = planned();
    sent(&mut log, &json!([question("q1")]), 2);
    let block = log.find(&handle("q1")).unwrap().key;
    let given = |chosen: &[&str], words: Option<&str>| Reply {
        block,
        given: Given::Answer {
            chosen: chosen.iter().map(|label| (*label).to_owned()).collect(),
            words: words.map(str::to_owned),
            notes: None,
        },
    };
    for reply in [
        given(&["C"], None),
        given(&["A", "B"], None),
        given(&[], Some("mine")),
        given(&[], None),
    ] {
        let refused = log.answer(&[reply], &person(Channel::Page, 3)).unwrap_err();
        assert!(matches!(refused, Refused::Invalid { .. }), "{refused:?}");
    }
    assert_eq!(log.find(&handle("q1")).unwrap().responses, []);
}

#[test]
fn a_rejection_carries_the_person_s_reason() {
    let mut log = planned();
    sent(
        &mut log,
        &json!([{"id": "a1", "kind": "approval", "stage": "design", "title": "A", "markdown": "OK?"}]),
        2,
    );
    let refused = log
        .answer(
            &[outcome(&log, "a1", Verdict::Rejected, None)],
            &person(Channel::Page, 3),
        )
        .unwrap_err();
    assert!(matches!(refused, Refused::Invalid { .. }));
    log.answer(
        &[outcome(&log, "a1", Verdict::Rejected, Some("Too broad."))],
        &person(Channel::Page, 3),
    )
    .unwrap();
}

#[test]
fn a_plan_rewritten_after_its_outcome_has_none_until_decided_again() {
    let mut log = planned();
    let plan = |markdown: &str| json!([{"id": "p1", "kind": "plan", "stage": "design", "title": "P", "markdown": markdown, "planMode": true}]);
    sent(&mut log, &plan("First."), 2);
    log.answer(
        &[outcome(&log, "p1", Verdict::Approved, None)],
        &person(Channel::Page, 3),
    )
    .unwrap();
    assert!(log.find(&handle("p1")).unwrap().counted().is_some());
    write(&mut log, &plan("Second."), &context(&main_agent(), 4));
    assert!(log.find(&handle("p1")).unwrap().counted().is_none());
    log.answer(
        &[outcome(&log, "p1", Verdict::Approved, None)],
        &person(Channel::Terminal, 5),
    )
    .unwrap();
    assert_eq!(
        log.find(&handle("p1")).unwrap().counted().unwrap().channel,
        Channel::Terminal
    );
}

#[test]
fn a_rewritten_plan_sent_again_waits_for_a_new_outcome() {
    let mut log = planned();
    let plan = |markdown: &str| json!([{"id": "p1", "kind": "plan", "stage": "design", "title": "P", "markdown": markdown, "planMode": true}]);
    sent(&mut log, &plan("First."), 2);
    log.answer(
        &[outcome(&log, "p1", Verdict::Rejected, Some("Too wide."))],
        &person(Channel::Page, 3),
    )
    .unwrap();
    let again = sent(&mut log, &plan("Second."), 4);
    let state = |log: &yardlog::YardLog| log.submission(again).unwrap().state(log);
    assert_eq!(state(&log), State::Open);
    log.answer(
        &[outcome(&log, "p1", Verdict::Approved, None)],
        &person(Channel::Page, 5),
    )
    .unwrap();
    assert_eq!(state(&log), State::Answered);
}

#[test]
fn a_withdrawn_submission_stays_withdrawn_and_keeps_its_earlier_answers() {
    let mut log = planned();
    let key = sent(&mut log, &json!([question("q1"), question("q2")]), 2);
    answer(&mut log, "q1", "A", &person(Channel::Page, 3));
    log.withdraw(key, subagent(), at(4)).unwrap();
    answer(&mut log, "q2", "A", &person(Channel::Page, 5));
    let submission = log.submission(key).unwrap();
    assert_eq!(submission.state(&log), State::Withdrawn);
    let change = submission
        .changes
        .iter()
        .find(|c| c.what == Happened::Withdrawn)
        .unwrap();
    assert_eq!(
        (change.act.at, change.act.actor.clone()),
        (at(4), Actor::Agent(subagent()))
    );
    assert_eq!(log.find(&handle("q1")).unwrap().responses.len(), 1);
}

#[test]
fn withdrawing_a_submission_that_is_not_open_is_refused_with_its_state() {
    let mut log = planned();
    let key = sent(&mut log, &json!([question("q1")]), 2);
    answer(&mut log, "q1", "A", &person(Channel::Page, 3));
    let before = log.clone();
    let refused = log.withdraw(key, main_agent(), at(4)).unwrap_err();
    assert_eq!(
        refused,
        Refused::NotOpen {
            submission: key,
            state: State::Answered
        }
    );
    assert_eq!(log, before);
}

#[test]
fn closing_withdraws_every_open_submission_and_refuses_answers_after() {
    let mut log = planned();
    let key = sent(&mut log, &json!([question("q1")]), 2);
    log.close(main_agent(), at(3)).unwrap();
    assert_eq!(log.root.closed.as_ref().unwrap().at, at(3));
    assert_eq!(log.submission(key).unwrap().state(&log), State::Withdrawn);
    let block = log.find(&handle("q1")).unwrap().key;
    let reply = Reply {
        block,
        given: Given::Answer {
            chosen: vec!["A".to_owned()],
            words: None,
            notes: None,
        },
    };
    assert_eq!(
        log.answer(&[reply], &person(Channel::Terminal, 4))
            .unwrap_err(),
        Refused::Closed
    );
}

#[test]
fn a_main_agent_of_another_session_takes_up_an_open_submission() {
    let mut log = planned();
    let key = sent(&mut log, &json!([question("q1")]), 2);
    assert_eq!(
        log.take_up(key, subagent(), at(3)).unwrap_err(),
        Refused::NotMain
    );
    log.take_up(key, other_session(), at(4)).unwrap();
    let submission = log.submission(key).unwrap();
    assert_eq!(submission.sender(), &Actor::Agent(other_session()));
    assert!(
        submission
            .changes
            .iter()
            .any(|c| c.what == Happened::TakenUp && c.act.at == at(4))
    );
}

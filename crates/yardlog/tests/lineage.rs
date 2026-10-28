//! A session that continues another counts as the first session of its chain wherever Yard Log
//! compares agents: it owns the stages and holds the tasks of the session it continues, while
//! every Act keeps the session id that made it.

use serde_json::{Value, json};
use yardlog::body::TaskState;
use yardlog::task_change::{TaskChange, TaskPatch};
use yardlog::write::Refusal;
use yardlog::{Agent, Lineage, YardLog};

use crate::common::{context, handle, main_agent, opened, try_write, write};

/// The main agent of `session-c`, which continues `session-b`, which continues `session-a`.
fn continuing() -> Agent {
    Agent::main("session-c")
}

fn chain() -> Lineage {
    Lineage::read(Some(
        br#"{"session-b": "session-a", "session-c": "session-b"}"#,
    ))
}

fn worker(id: &str) -> Agent {
    Agent::sub("session-a", id, "worker")
}

fn assignee(who: &Agent) -> Value {
    match &who.subagent {
        Some(sub) => {
            json!({"host": who.host, "session": who.session, "agent": sub.id, "agentType": sub.kind})
        }
        None => json!({"host": who.host, "session": who.session}),
    }
}

fn task(id: &str, progress: &str, who: &Agent) -> Value {
    json!({"id": id, "kind": "task", "stage": "build", "title": id, "state": progress, "assignee": assignee(who)})
}

/// Stage `build`, active and `session-a`'s main agent's, with `mine` assigned to that agent and
/// `theirs` to its worker; with the chain where `chained`.
fn prepared(chained: bool) -> YardLog {
    let mut log = opened();
    write(
        &mut log,
        &json!([{"id": "build", "kind": "stage", "title": "Build", "status": "active"}]),
        &context(&main_agent(), 1),
    );
    write(
        &mut log,
        &json!([
            task("mine", "open", &main_agent()),
            task("theirs", "open", &worker("first"))
        ]),
        &context(&main_agent(), 2),
    );
    if chained {
        log.lineage = chain();
    }
    log
}

#[test]
fn a_chain_of_continuations_reaches_back_to_its_first_session() {
    let lineage = chain();
    assert_eq!(lineage.first("claude-code", "session-c"), "session-a");
    assert_eq!(lineage.first("claude-code", "session-a"), "session-a");
    assert_eq!(lineage.first("codex", "session-c"), "session-c");
    assert_eq!(
        lineage.continued_by("claude-code", "session-a").as_deref(),
        Some("session-b")
    );
}

#[test]
fn a_cycle_in_the_continuations_ends_the_chain_and_bytes_that_are_not_an_object_hold_none() {
    let cycle = Lineage::read(Some(
        br#"{"session-b": "session-a", "session-a": "session-b", "x": 1}"#,
    ));
    assert_eq!(cycle.first("claude-code", "session-b"), "session-a");
    assert_eq!(Lineage::read(Some(b"[1, 2]")), Lineage::default());
    assert_eq!(Lineage::read(None), Lineage::default());
}

#[test]
fn agents_of_one_chain_are_one_agent_only_with_the_same_host_and_subagent() {
    let lineage = chain();
    assert!(lineage.same(&main_agent(), &continuing()));
    assert!(lineage.same(&worker("w"), &Agent::sub("session-c", "w", "other")));
    assert!(!lineage.same(&worker("w"), &continuing()));
    assert!(!lineage.same(&main_agent(), &Agent::main_in("codex", "session-c")));
    assert!(!lineage.same(&main_agent(), &Agent::main("session-z")));
}

#[test]
fn the_continuing_session_works_in_the_stage_the_session_it_continues_made_active() {
    let log = prepared(true);
    let build = log.stages[0].stage.key.stage();
    assert_eq!(log.working_stage(&continuing(), None), Some(build));
    assert_eq!(log.active_stages_of(&continuing()).count(), 1);
    assert_eq!(prepared(false).working_stage(&continuing(), None), None);
}

#[test]
fn a_task_assigned_to_the_ended_session_counts_as_assigned_to_the_one_that_continues_it() {
    let log = prepared(true);
    let agent = continuing();
    let held: Vec<&str> = log
        .assigned_to(&agent)
        .map(|(_, block, _)| block.id.as_str())
        .collect();
    assert_eq!(held, ["mine"]);
    assert_eq!(prepared(false).assigned_to(&continuing()).count(), 0);
}

#[test]
fn the_continuing_session_reassigns_a_task_of_the_stage_as_its_owner() {
    let batch = json!([task("theirs", "open", &worker("second"))]);
    let mut log = prepared(true);
    try_write(&mut log, &batch, &context(&continuing(), 3)).unwrap();
    let mut alone = prepared(false);
    assert!(matches!(
        try_write(&mut alone, &batch, &context(&continuing(), 3)),
        Err(Refusal::Faults(_))
    ));
}

#[test]
fn a_mirror_by_the_continuing_session_changes_the_task_assigned_to_the_ended_one() {
    let patch = TaskPatch {
        state: Some(TaskState::Done),
        mirror: true,
        ..TaskPatch::default()
    };
    let mut log = prepared(true);
    let change = log
        .change_task(&handle("mine"), &patch, &context(&continuing(), 3))
        .unwrap();
    assert!(matches!(change, TaskChange::Changed { .. }));
    let mut alone = prepared(false);
    let unchanged = alone
        .change_task(&handle("mine"), &patch, &context(&continuing(), 3))
        .unwrap();
    assert_eq!(unchanged, TaskChange::Unchanged);
}

#[test]
fn a_second_stage_made_active_by_the_continuing_session_is_refused_as_for_one_agent() {
    let batch = json!([{"id": "review", "kind": "stage", "title": "Review", "status": "active"}]);
    let mut log = prepared(true);
    let Err(Refusal::Faults(faults)) = try_write(&mut log, &batch, &context(&continuing(), 3))
    else {
        panic!("the write was not refused");
    };
    assert!(faults[0].message.contains("`build`"), "{faults:?}");
    try_write(&mut prepared(false), &batch, &context(&continuing(), 3)).unwrap();
}

#[test]
fn a_write_of_the_continuing_session_keeps_its_own_session_id() {
    let mut log = prepared(true);
    write(
        &mut log,
        &json!([task("mine", "active", &main_agent())]),
        &context(&continuing(), 3),
    );
    let version = log.find(&handle("mine")).unwrap().current();
    assert_eq!(
        version
            .act
            .actor
            .agent()
            .map(|agent| agent.session.as_str()),
        Some("session-c")
    );
}

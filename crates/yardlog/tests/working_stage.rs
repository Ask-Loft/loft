//! The one stage each agent works in, whichever part of Yard Client asks for it.

use serde_json::{Value, json};
use yardlog::{Agent, StageKey, YardLog};

use crate::common::{context, main_agent, opened, other_session, write};

fn worker() -> Agent {
    Agent::sub("session-a", "worker-1", "worker")
}

fn stage(id: &str, status: &str) -> Value {
    json!({"id": id, "kind": "stage", "title": id, "status": status})
}

fn task(id: &str, stage: &str, progress: &str, who: &Agent) -> Value {
    let sub = who.subagent.as_ref().unwrap();
    json!({
        "id": id, "kind": "task", "stage": stage, "title": id, "state": progress,
        "assignee": {"host": who.host, "session": who.session, "agent": sub.id, "agentType": sub.kind}
    })
}

fn key(log: &YardLog, id: &str) -> StageKey {
    log.stages
        .iter()
        .find(|file| file.stage.id.as_str() == id)
        .map(|file| file.stage.key.stage())
        .unwrap()
}

/// Stages `build` (active, the main agent's) and `review` (planned), with the worker assigned a
/// task in each: by the main agent in `build`, and by its own claim in `review`.
fn delegated() -> YardLog {
    let mut log = opened();
    write(
        &mut log,
        &json!([stage("build", "active"), stage("review", "planned")]),
        &context(&main_agent(), 1),
    );
    write(
        &mut log,
        &json!([
            task("compile", "build", "open", &worker()),
            {"id": "read", "kind": "task", "stage": "review", "title": "read", "state": "open"}
        ]),
        &context(&main_agent(), 2),
    );
    write(
        &mut log,
        &json!([task("read", "review", "open", &worker())]),
        &context(&worker(), 2),
    );
    log
}

#[test]
fn an_agent_whose_active_assigned_tasks_lie_in_two_stages_has_no_working_stage() {
    let mut log = delegated();
    write(
        &mut log,
        &json!([
            task("compile", "build", "active", &worker()),
            task("read", "review", "active", &worker())
        ]),
        &context(&worker(), 3),
    );
    assert_eq!(log.working_stage(&worker(), None), None);
    assert_eq!(log.starting_stage(&worker(), None), None);
    assert_eq!(
        log.working_stage(&worker(), Some("read")),
        Some(key(&log, "review"))
    );
}

#[test]
fn a_main_agent_with_no_active_stage_and_no_assigned_task_has_no_working_stage() {
    let mut log = opened();
    write(
        &mut log,
        &json!([stage("theirs", "active")]),
        &context(&other_session(), 1),
    );
    assert_eq!(log.working_stage(&main_agent(), None), None);
    assert_eq!(
        log.working_stage(&other_session(), None),
        Some(key(&log, "theirs"))
    );
}

#[test]
fn the_working_stage_is_the_named_task_s_then_the_assigned_one_then_its_own_then_its_main_agent_s()
{
    let mut log = delegated();
    let build = key(&log, "build");
    let review = key(&log, "review");
    // No active assignment and no stage of its own: the session's main agent's stage.
    assert_eq!(log.working_stage(&worker(), None), Some(build));
    // A named task assigned to the agent gives its stage, active or not.
    assert_eq!(log.working_stage(&worker(), Some("read")), Some(review));
    // A named task not assigned to the agent gives none, whatever else it has.
    assert_eq!(log.working_stage(&worker(), Some("missing")), None);
    assert_eq!(log.working_stage(&main_agent(), Some("compile")), None);
    // The one stage its active assigned tasks lie in comes before its own.
    write(
        &mut log,
        &json!([stage("own", "active")]),
        &context(&worker(), 3),
    );
    let own = key(&log, "own");
    assert_eq!(log.working_stage(&worker(), None), Some(own));
    write(
        &mut log,
        &json!([task("read", "review", "active", &worker())]),
        &context(&worker(), 4),
    );
    assert_eq!(log.working_stage(&worker(), None), Some(review));
    write(
        &mut log,
        &json!([task("read", "review", "done", &worker())]),
        &context(&worker(), 5),
    );
    assert_eq!(log.working_stage(&worker(), None), Some(own));
    // A subagent's run starts in its main agent's stage, whatever stage it owns.
    assert_eq!(log.starting_stage(&worker(), None), Some(build));
    assert_eq!(log.working_stage(&main_agent(), None), Some(build));
}

#[test]
fn an_agent_s_own_stage_is_never_its_main_agent_s() {
    let log = delegated();
    let idle = Agent::sub("session-a", "idle-1", "worker");
    assert_eq!(log.working_stage(&idle, None), Some(key(&log, "build")));
    assert_eq!(log.own_stage(&idle, None), None);
}

#[test]
fn an_agent_s_own_stage_is_its_assigned_task_s_or_the_stage_it_opened() {
    let mut log = delegated();
    write(
        &mut log,
        &json!([task("read", "review", "active", &worker())]),
        &context(&worker(), 3),
    );
    assert_eq!(log.own_stage(&worker(), None), Some(key(&log, "review")));
    let opener = Agent::sub("session-a", "opener-1", "worker");
    write(
        &mut log,
        &json!([stage("mine", "active")]),
        &context(&opener, 4),
    );
    assert_eq!(log.own_stage(&opener, None), Some(key(&log, "mine")));
}

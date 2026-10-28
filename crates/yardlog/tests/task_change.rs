//! A task changed in place: the fields given applied to the task as Yard Log holds it, and an
//! agent's active tasks released.

use serde_json::{Value, json};
use yardlog::body::{Body, Task, TaskState, TaskType};
use yardlog::task_change::{TaskChange, TaskPatch};
use yardlog::write::Refusal;
use yardlog::{Agent, YardLog};

use crate::common::{context, handle, main_agent, opened, write};

fn worker(id: &str) -> Agent {
    Agent::sub("session-a", id, "worker")
}

fn assigned(id: &str, progress: &str, who: &Agent) -> Value {
    let sub = who.subagent.as_ref().unwrap();
    json!({
        "id": id, "kind": "task", "stage": "build", "title": id, "state": progress,
        "detail": "As planned", "type": "fix",
        "assignee": {"host": who.host, "session": who.session, "agent": sub.id, "agentType": sub.kind}
    })
}

/// Stage `build`, the main agent's, with `one` and `two` assigned to the worker `first`.
fn prepared() -> YardLog {
    let mut log = opened();
    write(
        &mut log,
        &json!([{"id": "build", "kind": "stage", "title": "Build", "status": "active"}]),
        &context(&main_agent(), 1),
    );
    write(
        &mut log,
        &json!([
            assigned("one", "open", &worker("first")),
            assigned("two", "open", &worker("first"))
        ]),
        &context(&main_agent(), 2),
    );
    log
}

fn task(log: &YardLog, id: &str) -> Task {
    match &log.find(&handle(id)).unwrap().current().body {
        Body::Task(task) => task.clone(),
        body => panic!("{id} is {body:?}"),
    }
}

fn state(progress: TaskState) -> TaskPatch {
    TaskPatch {
        state: Some(progress),
        ..TaskPatch::default()
    }
}

#[test]
fn a_changed_task_keeps_its_stage_assignee_and_every_field_not_given() {
    let mut log = prepared();
    let first = worker("first");
    let before = task(&log, "one");
    let change = log
        .change_task(
            &handle("one"),
            &state(TaskState::Active),
            &context(&first, 3),
        )
        .unwrap();
    assert!(matches!(change, TaskChange::Changed { ref tasks, .. } if tasks == &[handle("one")]));
    let after = task(&log, "one");
    assert_eq!(
        after,
        Task {
            state: TaskState::Active,
            ..before
        }
    );
    assert_eq!(after.assignee, Some(first.clone()));
    assert_eq!(after.kind, TaskType::Fix);
    assert_eq!(log.stages[0].blocks[0].id, handle("one"));
    assert_eq!(log.stages[0].blocks[0].versions.len(), 2);
    let patch = TaskPatch {
        title: Some("Build one".to_owned()),
        detail: Some("Done in abc".to_owned()),
        kind: Some(TaskType::Regular),
        ..TaskPatch::default()
    };
    log.change_task(&handle("one"), &patch, &context(&first, 4))
        .unwrap();
    let after = task(&log, "one");
    assert_eq!(
        (after.title.as_str(), after.detail.as_deref(), after.kind),
        ("Build one", Some("Done in abc"), TaskType::Regular)
    );
    assert_eq!(after.state, TaskState::Active);
}

#[test]
fn a_change_applies_to_the_task_as_it_stands_without_a_revision_to_base_it_on() {
    let mut log = prepared();
    let first = worker("first");
    let mut stale = context(&first, 3);
    stale.base = Some(0);
    let renamed = TaskPatch {
        title: Some("Renamed".to_owned()),
        ..TaskPatch::default()
    };
    log.change_task(&handle("one"), &renamed, &context(&main_agent(), 3))
        .unwrap();
    log.change_task(&handle("one"), &state(TaskState::Done), &stale)
        .unwrap();
    let after = task(&log, "one");
    assert_eq!(
        (after.title.as_str(), after.state),
        ("Renamed", TaskState::Done)
    );
}

#[test]
fn a_change_that_leaves_the_task_as_it_was_writes_nothing() {
    let mut log = prepared();
    let before = log.clone();
    let patch = TaskPatch {
        state: Some(TaskState::Open),
        title: Some("one".to_owned()),
        detail: Some("As planned".to_owned()),
        kind: Some(TaskType::Fix),
        mirror: false,
    };
    let change = log
        .change_task(&handle("one"), &patch, &context(&worker("first"), 3))
        .unwrap();
    assert_eq!(change, TaskChange::Unchanged);
    assert_eq!(log, before);
}

#[test]
fn a_change_the_delegation_rules_refuse_changes_no_block() {
    let mut log = prepared();
    let before = log.clone();
    let refused = log.change_task(
        &handle("one"),
        &state(TaskState::Done),
        &context(&worker("other"), 3),
    );
    assert!(matches!(refused, Err(Refusal::Faults(_))), "{refused:?}");
    assert_eq!(log, before);
}

#[test]
fn a_change_of_an_id_that_names_no_task_is_refused_naming_the_id() {
    let mut log = prepared();
    let before = log.clone();
    for id in ["missing", "build"] {
        let Err(Refusal::Faults(faults)) = log.change_task(
            &handle(id),
            &state(TaskState::Done),
            &context(&main_agent(), 3),
        ) else {
            panic!("{id} is changed");
        };
        assert_eq!(faults[0].block, id);
        assert!(faults[0].message.contains(&format!("`{id}`")));
    }
    assert_eq!(log, before);
}

fn mirrored(progress: TaskState) -> TaskPatch {
    TaskPatch {
        mirror: true,
        ..state(progress)
    }
}

#[test]
fn a_mirrored_change_of_a_task_assigned_to_another_agent_writes_nothing() {
    let mut log = prepared();
    let before = log.clone();
    let change = log
        .change_task(
            &handle("one"),
            &mirrored(TaskState::Done),
            &context(&worker("other"), 3),
        )
        .unwrap();
    assert_eq!(change, TaskChange::Unchanged);
    assert_eq!(log, before);
}

#[test]
fn a_mirrored_change_of_an_unassigned_task_of_a_stage_the_agent_does_not_own_writes_nothing() {
    let mut log = prepared();
    write(
        &mut log,
        &json!([{"id": "free", "kind": "task", "stage": "build", "title": "Free", "state": "open"}]),
        &context(&main_agent(), 3),
    );
    let before = log.clone();
    let change = log
        .change_task(
            &handle("free"),
            &mirrored(TaskState::Active),
            &context(&worker("first"), 4),
        )
        .unwrap();
    assert_eq!(change, TaskChange::Unchanged);
    assert_eq!(log, before);
    log.change_task(
        &handle("free"),
        &mirrored(TaskState::Active),
        &context(&main_agent(), 5),
    )
    .unwrap();
    assert_eq!(task(&log, "free").state, TaskState::Active);
}

#[test]
fn a_mirrored_change_of_a_done_or_dropped_task_keeps_its_state_and_applies_its_other_fields() {
    let mut log = prepared();
    let first = worker("first");
    write(
        &mut log,
        &json!([
            assigned("one", "done", &first),
            assigned("two", "dropped", &first)
        ]),
        &context(&main_agent(), 3),
    );
    for (id, kept) in [("one", TaskState::Done), ("two", TaskState::Dropped)] {
        let patch = TaskPatch {
            title: Some(format!("Renamed {id}")),
            ..mirrored(TaskState::Active)
        };
        log.change_task(&handle(id), &patch, &context(&first, 4))
            .unwrap();
        let after = task(&log, id);
        assert_eq!((after.state, after.title), (kept, format!("Renamed {id}")));
    }
    let unchanged = log
        .change_task(
            &handle("one"),
            &mirrored(TaskState::Open),
            &context(&first, 5),
        )
        .unwrap();
    assert_eq!(unchanged, TaskChange::Unchanged);
}

#[test]
fn releasing_returns_the_agent_s_active_tasks_to_open_keeping_their_assignee() {
    let mut log = prepared();
    let first = worker("first");
    write(
        &mut log,
        &json!([
            assigned("one", "active", &first),
            assigned("two", "active", &first)
        ]),
        &context(&first, 3),
    );
    let revision = log.root.revision;
    let TaskChange::Changed { written, tasks } = log.release(&first, &context(&first, 4)).unwrap()
    else {
        panic!("nothing released");
    };
    assert_eq!(tasks, [handle("one"), handle("two")]);
    assert_eq!(written.revision, revision + 1);
    for id in ["one", "two"] {
        let after = task(&log, id);
        assert_eq!(after.state, TaskState::Open);
        assert_eq!(after.assignee, Some(first.clone()));
    }
}

#[test]
fn releasing_leaves_the_agent_s_finished_and_open_tasks_and_other_agents_tasks_as_they_were() {
    let mut log = prepared();
    let first = worker("first");
    let second = worker("second");
    write(
        &mut log,
        &json!([
            assigned("one", "done", &first),
            assigned("two", "open", &first),
            assigned("three", "dropped", &first),
            assigned("four", "active", &first)
        ]),
        &context(&main_agent(), 3),
    );
    write(
        &mut log,
        &json!([assigned("theirs", "active", &second)]),
        &context(&main_agent(), 4),
    );
    let before = log.clone();
    log.release(&first, &context(&first, 5)).unwrap();
    for id in ["one", "two", "three", "theirs"] {
        let block = log.find(&handle(id)).unwrap();
        assert_eq!(block, before.find(&handle(id)).unwrap(), "{id}");
    }
    assert_eq!(task(&log, "four").state, TaskState::Open);
}

#[test]
fn releasing_an_agent_with_no_active_task_writes_nothing() {
    let mut log = prepared();
    let before = log.clone();
    let change = log
        .release(&worker("first"), &context(&worker("first"), 3))
        .unwrap();
    assert_eq!(change, TaskChange::Unchanged);
    assert_eq!(log, before);
}

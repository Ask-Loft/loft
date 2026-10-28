//! Assigned tasks retain their stage and identity across agents and interrupted attempts.

use serde_json::{Value, json};
use yardlog::body::Body;
use yardlog::write::Refusal;
use yardlog::{Agent, YardLog};

use crate::common::{at, context, handle, main_agent, opened, try_write, write};

fn worker(id: &str) -> Agent {
    Agent::sub("session-a", id, "worker")
}

fn assigned(id: &str, state: &str, who: Option<&Agent>) -> Value {
    let mut task = json!({"id":id,"kind":"task","stage":"build","title":"Build it","state":state});
    if let Some(who) = who {
        let sub = who.subagent.as_ref().unwrap();
        task["assignee"] =
            json!({"host":who.host,"session":who.session,"agent":sub.id,"agentType":sub.kind});
    }
    task
}

fn prepared() -> YardLog {
    let mut log = opened();
    write(
        &mut log,
        &json!([{"id":"build","kind":"stage","title":"Build","status":"active"}]),
        &context(&main_agent(), 1),
    );
    write(
        &mut log,
        &json!([assigned("one", "open", None), assigned("two", "open", None)]),
        &context(&main_agent(), 2),
    );
    log
}

#[test]
fn workers_claim_and_run_parallel_tasks_without_taking_the_stage() {
    let mut log = prepared();
    let keys: Vec<_> = log.stages[0].blocks.iter().map(|block| block.key).collect();
    for (id, who) in [("one", worker("first")), ("two", worker("second"))] {
        write(
            &mut log,
            &json!([assigned(id, "active", Some(&who))]),
            &context(&who, 3),
        );
        assert_eq!(log.active_stages_of(&who).count(), 0);
    }
    assert_eq!(log.stages[0].stage.owner(), Some(&main_agent()));
    assert_eq!(
        log.stages[0]
            .blocks
            .iter()
            .map(|block| block.key)
            .collect::<Vec<_>>(),
        keys
    );
    assert!(
        log.stages[0]
            .blocks
            .iter()
            .all(|block| block.versions.len() == 2)
    );
    write(
        &mut log,
        &json!([
            {"id":"build","kind":"stage","title":"Build","status":"planned"},
            {"id":"review","kind":"stage","title":"Review","status":"active"}
        ]),
        &context(&main_agent(), 4),
    );
    write(
        &mut log,
        &json!([assigned("one", "done", Some(&worker("first")))]),
        &context(&worker("first"), 5),
    );
    assert_eq!(log.stages[0].blocks[0].key, keys[0]);
    assert_eq!(log.stages[1].blocks.len(), 0);
    let files: std::collections::BTreeMap<_, _> = log.files().into_iter().collect();
    let read = YardLog::read(files[yardlog::codec::ROOT].as_bytes(), |name| {
        files.get(name).map(|s| s.as_bytes().to_vec())
    })
    .unwrap();
    assert_eq!(read, log);
}

#[test]
fn reassignment_rejects_stale_and_fresh_updates_from_the_former_worker_atomically() {
    let mut log = prepared();
    let first = worker("first");
    let second = worker("second");
    write(
        &mut log,
        &json!([assigned("one", "active", Some(&first))]),
        &context(&first, 3),
    );
    let base = log.root.revision;
    write(
        &mut log,
        &json!([assigned("one", "active", Some(&second))]),
        &context(&main_agent(), 4),
    );
    let before = log.clone();
    let mut stale = context(&first, 5);
    stale.base = Some(base);
    assert!(matches!(
        try_write(
            &mut log,
            &json!([assigned("one", "done", Some(&first))]),
            &stale
        ),
        Err(Refusal::Stale { .. })
    ));
    assert!(
        try_write(
            &mut log,
            &json!([
                assigned("one", "done", Some(&second)),
                {"id":"unwanted","kind":"note","stage":"build","markdown":"Never saved"}
            ]),
            &context(&first, 6)
        )
        .is_err()
    );
    assert_eq!(log, before);
    write(
        &mut log,
        &json!([assigned("one", "done", Some(&second))]),
        &context(&second, 7),
    );
}

#[test]
fn releasing_an_assignment_keeps_protection_and_allows_a_new_self_claim() {
    let mut log = prepared();
    let first = worker("first");
    write(
        &mut log,
        &json!([assigned("one", "active", Some(&first))]),
        &context(&first, 3),
    );
    assert!(
        try_write(
            &mut log,
            &json!([assigned("one", "open", Some(&worker("second")))]),
            &context(&worker("other"), 4)
        )
        .is_err()
    );
    write(
        &mut log,
        &json!([assigned("one", "open", None)]),
        &context(&first, 5),
    );
    assert!(
        try_write(
            &mut log,
            &json!([assigned("one", "done", None)]),
            &context(&worker("other"), 6)
        )
        .is_err()
    );
    write(
        &mut log,
        &json!([assigned("one", "active", Some(&worker("second")))]),
        &context(&worker("second"), 7),
    );
    // Legacy tasks remain cooperative even when another caller did not create them.
    write(
        &mut log,
        &json!([assigned("two", "done", None)]),
        &context(&worker("other"), 8),
    );
}

#[test]
fn task_execution_spans_end_on_pause_and_handoff_then_resume_separately() {
    let mut log = prepared();
    let first = worker("first");
    let second = worker("second");
    for (seconds, state, who) in [
        (3, "active", &first),
        (4, "open", &first),
        (5, "active", &first),
        (6, "active", &second),
        (8, "done", &second),
    ] {
        write(
            &mut log,
            &json!([assigned("one", state, Some(who))]),
            &context(&main_agent(), seconds),
        );
    }
    let task = log.find(&handle("one")).unwrap();
    let spans = task.executions();
    assert_eq!(
        spans
            .iter()
            .map(|s| (s.started, s.ended))
            .collect::<Vec<_>>(),
        vec![
            (at(3), Some(at(4))),
            (at(5), Some(at(6))),
            (at(6), Some(at(8)))
        ]
    );
    assert_eq!(spans[2].assignee.as_ref(), Some(&second));
    assert_eq!(task.times().started, Some(at(3)));
}

#[test]
fn assignment_identity_ignores_the_descriptive_agent_type() {
    let mut log = prepared();
    let first = worker("first");
    write(
        &mut log,
        &json!([assigned("one", "active", Some(&first))]),
        &context(&first, 3),
    );
    let renamed = Agent::sub("session-a", "first", "reviewer");
    write(
        &mut log,
        &json!([assigned("one", "done", Some(&first))]),
        &context(&renamed, 4),
    );
    assert!(
        matches!(&log.find(&handle("one")).unwrap().current().body, Body::Task(task) if task.assignee.as_ref() == Some(&first))
    );
}

#[test]
fn malformed_assignments_are_refused_before_a_batch_is_applied() {
    for who in [
        json!([]),
        json!({"host":"codex"}),
        json!({"host":"../bad","session":"s"}),
        json!({"host":"codex","session":"s","agent":"a"}),
        json!({"host":"codex","session":"s","name":"display"}),
    ] {
        let mut task = assigned("one", "open", None);
        task["assignee"] = who;
        assert!(yardlog::input::read_batch(&json!([task])).is_err());
    }
}

#[test]
fn unassigned_tasks_keep_the_legacy_canonical_shape() {
    let log = prepared();
    let files: std::collections::BTreeMap<_, _> = log.files().into_iter().collect();
    assert!(files.values().all(|text| !text.contains("assignee:")));
    let read = YardLog::read(files[yardlog::codec::ROOT].as_bytes(), |name| {
        files.get(name).map(|text| text.as_bytes().to_vec())
    })
    .unwrap();
    assert_eq!(read, log);
    assert!(
        matches!(&read.stages[0].blocks[0].current().body, Body::Task(task) if task.assignee.is_none())
    );
}

#[test]
fn merging_parallel_assignments_keeps_both_tasks_and_their_histories() {
    let base = prepared();
    let mut ours = base.clone();
    let mut theirs = base.clone();
    write(
        &mut ours,
        &json!([assigned("one", "active", Some(&worker("first")))]),
        &context(&main_agent(), 3),
    );
    write(
        &mut theirs,
        &json!([assigned("two", "active", Some(&worker("second")))]),
        &context(&main_agent(), 4),
    );
    let merged =
        yardlog::merge::merge_stage(Some(&base.stages[0]), &ours.stages[0], &theirs.stages[0]);
    for (index, name) in ["first", "second"].iter().enumerate() {
        let task = &merged.blocks[index];
        assert_eq!(task.key, base.stages[0].blocks[index].key);
        assert_eq!(task.versions.len(), 2);
        assert!(
            matches!(&task.current().body, Body::Task(body) if body.assignee.as_ref() == Some(&worker(name)))
        );
    }
}

#[test]
fn making_active_a_stage_another_agent_owns_grants_no_ownership_of_the_same_write_s_tasks() {
    let mut log = prepared();
    let first = worker("first");
    let intruder = worker("intruder");
    write(
        &mut log,
        &json!([assigned("one", "active", Some(&first))]),
        &context(&first, 3),
    );
    let before = log.clone();
    let stage = json!({"id":"build","kind":"stage","title":"Build","status":"active"});
    for task in [
        assigned("one", "active", Some(&intruder)),
        assigned("one", "done", Some(&first)),
    ] {
        assert!(
            try_write(&mut log, &json!([stage, task]), &context(&intruder, 4)).is_err(),
            "{task}"
        );
        assert_eq!(log, before);
    }
}

#[test]
fn making_active_a_stage_no_agent_owns_grants_ownership_of_the_same_write_s_tasks() {
    let mut log = prepared();
    let first = worker("first");
    let second = worker("second");
    write(
        &mut log,
        &json!([assigned("one", "active", Some(&first))]),
        &context(&first, 3),
    );
    write(
        &mut log,
        &json!([{"id":"build","kind":"stage","title":"Build","status":"planned"}]),
        &context(&main_agent(), 4),
    );
    write(
        &mut log,
        &json!([
            {"id":"build","kind":"stage","title":"Build","status":"active"},
            assigned("one", "open", Some(&second))
        ]),
        &context(&second, 5),
    );
    assert_eq!(log.stages[0].stage.owner(), Some(&second));
}

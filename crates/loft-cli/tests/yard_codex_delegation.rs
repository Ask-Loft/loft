//! Explicit Codex task references continue existing assignments without a worker-owned stage.

use serde_json::{Value, json};

use crate::yard::{Yard, block, document};
use crate::yard_codex::{event, hook};

fn setup() -> Yard {
    let yard = Yard::new();
    yard.write("codex-session", r#"[
      {"id":"build","kind":"stage","title":"Build","status":"active"},
      {"id":"shared","kind":"task","stage":"build","title":"Original scope","detail":"Keep detail","type":"fix","state":"open",
       "assignee":{"host":"codex","session":"codex-session","agent":"worker","agentType":"builder"}}
    ]"#, &["--host","codex"]);
    yard
}

fn worker(yard: &Yard, name: &str) -> Value {
    let mut input = event(yard, name);
    input["agent_id"] = json!("worker");
    input["agent_type"] = json!("builder");
    input
}

fn plan(yard: &Yard, steps: &[(&str, &str)]) -> Value {
    let mut input = worker(yard, "PostToolUse");
    input["tool_name"] = json!("update_plan");
    input["tool_response"] = json!("Plan updated");
    input["tool_input"] = json!({"plan":steps.iter().map(|(step,status)|json!({"step":step,"status":status})).collect::<Vec<_>>()});
    input
}

fn read(yard: &Yard) -> Value {
    yard.read("codex-session", &["--host", "codex", "--mark-read"])
}

#[test]
fn bound_codex_steps_update_the_existing_task_without_owning_its_stage() {
    let yard = setup();
    let before = read(&yard);
    hook(
        &yard,
        &plan(
            &yard,
            &[
                ("[yard:shared] Local wording", "in_progress"),
                ("Local checklist", "pending"),
            ],
        ),
    );
    let after = read(&yard);
    assert_eq!(after["stages"].as_array().unwrap().len(), 1);
    // The plain step is mirrored beside the bound task, and no second task is made for it.
    assert_eq!(after["stages"][0]["blocks"].as_array().unwrap().len(), 2);
    let task = block(&after, "shared");
    assert_eq!(task["key"], block(&before, "shared")["key"]);
    assert_eq!(task["version"]["body"]["title"], "Original scope");
    assert_eq!(task["version"]["body"]["detail"], "Keep detail");
    assert_eq!(task["version"]["body"]["type"], "fix");
    assert_eq!(task["version"]["body"]["assignee"]["agent"], "worker");
    assert_eq!(task["version"]["body"]["state"], "active");
    assert_eq!(after["stages"][0]["owner"], before["stages"][0]["owner"]);
    hook(
        &yard,
        &plan(
            &yard,
            &[
                ("[yard:shared] Another wording", "in_progress"),
                ("Local checklist", "pending"),
            ],
        ),
    );
    assert_eq!(read(&yard)["revision"], after["revision"]);
    hook(&yard, &plan(&yard, &[("[yard:shared] Done", "completed")]));
    assert_eq!(
        block(&read(&yard), "shared")["version"]["body"]["state"],
        "done"
    );
}

#[test]
fn invalid_unknown_duplicate_and_removed_bindings_never_create_or_drop_a_task() {
    let yard = setup();
    let before = read(&yard)["revision"].clone();
    for steps in [
        vec![("[yard:missing] Unknown", "pending")],
        vec![("[yard:../bad] Wrong", "pending")],
        vec![("[yard:shared Missing bracket", "pending")],
        vec![
            ("[yard:shared] One", "pending"),
            ("[yard:shared] Two", "completed"),
        ],
        vec![],
    ] {
        hook(&yard, &plan(&yard, &steps));
        assert_eq!(read(&yard)["revision"], before);
    }
}

#[test]
fn a_parent_plan_cannot_reset_another_assignees_bound_progress() {
    let yard = setup();
    hook(
        &yard,
        &plan(&yard, &[("[yard:shared] Work", "in_progress")]),
    );
    let before = read(&yard)["revision"].clone();
    let mut parent = plan(&yard, &[("[yard:shared] Wait", "pending")]);
    parent.as_object_mut().unwrap().remove("agent_id");
    parent.as_object_mut().unwrap().remove("agent_type");
    hook(&yard, &parent);
    assert_eq!(read(&yard)["revision"], before);
}

#[test]
fn worker_end_opens_unfinished_tasks_and_late_progress_waits_for_a_new_start() {
    let yard = setup();
    hook(
        &yard,
        &plan(&yard, &[("[yard:shared] Work", "in_progress")]),
    );
    hook(&yard, &worker(&yard, "SubagentStop"));
    let ended = read(&yard);
    assert_eq!(block(&ended, "shared")["version"]["body"]["state"], "open");
    assert_eq!(
        block(&ended, "shared")["version"]["body"]["assignee"]["agent"],
        "worker"
    );
    hook(
        &yard,
        &plan(&yard, &[("[yard:shared] Late", "in_progress")]),
    );
    assert_eq!(read(&yard)["revision"], ended["revision"]);
    hook(&yard, &worker(&yard, "SubagentStart"));
    hook(
        &yard,
        &plan(&yard, &[("[yard:shared] Resume", "in_progress")]),
    );
    assert_eq!(
        block(&read(&yard), "shared")["version"]["body"]["state"],
        "active"
    );
    hook(&yard, &worker(&yard, "Interrupt"));
    assert_eq!(
        block(&read(&yard), "shared")["version"]["body"]["state"],
        "open"
    );
}

#[test]
fn worker_end_and_automatic_mirrors_preserve_finished_or_reassigned_tasks() {
    let yard = setup();
    hook(&yard, &plan(&yard, &[("[yard:shared] Done", "completed")]));
    let before = read(&yard)["revision"].clone();
    hook(&yard, &plan(&yard, &[("[yard:shared] Reopen", "pending")]));
    hook(&yard, &worker(&yard, "SubagentStop"));
    assert_eq!(read(&yard)["revision"], before);
    let mut task = block(&read(&yard), "shared")["version"]["body"].clone();
    task["assignee"]["agent"] = json!("other");
    task["state"] = json!("active");
    task["id"] = json!("shared");
    task["kind"] = json!("task");
    task["stage"] = json!("build");
    assert_written(&yard.write(
        "codex-session",
        &json!([task]).to_string(),
        &["--host", "codex"],
    ));
    let reassigned = read(&yard)["revision"].clone();
    // The former assignee's mirror, applied to the task as it now stands, changes nothing.
    hook(&yard, &worker(&yard, "SubagentStart"));
    hook(&yard, &plan(&yard, &[("[yard:shared] Mine", "completed")]));
    assert_eq!(read(&yard)["revision"], reassigned);
    hook(&yard, &worker(&yard, "SubagentStop"));
    assert_eq!(read(&yard)["revision"], reassigned);
}

#[test]
fn identified_worker_context_lists_its_recorded_assignment_in_a_bounded_message() {
    let yard = setup();
    let output = hook(&yard, &worker(&yard, "SubagentStart"));
    let text = output["hookOutput"]["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("assigned") && text.contains("shared") && text.contains("build"),
        "{text}"
    );
    assert!(text.chars().count() <= 2000);
    assert!(!text.contains("Active stage:"));
}

#[test]
fn an_ambiguous_end_cannot_open_another_actors_active_assignment() {
    let yard = setup();
    hook(
        &yard,
        &plan(&yard, &[("[yard:shared] Work", "in_progress")]),
    );
    hook(&yard, &worker(&yard, "SubagentStart"));
    let before = read(&yard)["revision"].clone();
    for name in ["Stop", "Interrupt", "SubagentStop"] {
        hook(&yard, &event(&yard, name));
        assert_eq!(read(&yard)["revision"], before);
    }
    // Even without a live-worker map, a child end lacking identity is not a main end.
    let mut task = block(&read(&yard), "shared")["version"]["body"].clone();
    task["assignee"] = json!({"host":"codex", "session":"codex-session"});
    task["id"] = json!("shared");
    task["kind"] = json!("task");
    task["stage"] = json!("build");
    assert_written(&yard.write(
        "codex-session",
        &json!([task]).to_string(),
        &["--host", "codex"],
    ));
    let before = read(&yard)["revision"].clone();
    hook(&yard, &event(&yard, "SubagentStop"));
    assert_eq!(read(&yard)["revision"], before);
}

#[test]
fn assignment_context_reports_omitted_tasks_and_directs_the_agent_to_read_yard_log() {
    let yard = setup();
    let tasks: Vec<_> = (0..35).map(|n|json!({"id":format!("task-{n}-{}", "long".repeat(8)),"kind":"task","stage":"build","title":"Work","state":"open","assignee":{"host":"codex","session":"codex-session","agent":"worker","agentType":"builder"}})).collect();
    read(&yard);
    assert_written(&yard.write(
        "codex-session",
        &json!(tasks).to_string(),
        &["--host", "codex"],
    ));
    let output = hook(&yard, &worker(&yard, "SubagentStart"));
    let text = output["hookOutput"]["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("Unfinished tasks assigned to you (36):"),
        "{text}"
    );
    assert!(text.contains("more: read Yard Log for them"), "{text}");
    let briefing = text.split("\nYard caller:").next().unwrap();
    assert!(briefing.chars().count() <= 2000);
}

#[test]
fn a_parent_unbound_plan_preserves_the_native_task_after_delegation_and_removal() {
    let yard = setup();
    let mut parent = plan(&yard, &[("Native scope", "pending")]);
    parent.as_object_mut().unwrap().remove("agent_id");
    parent.as_object_mut().unwrap().remove("agent_type");
    hook(&yard, &parent);
    let doc = read(&yard);
    let native = doc["stages"][0]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] != "shared")
        .unwrap();
    let id = native["id"].as_str().unwrap();
    let mut body = native["version"]["body"].clone();
    body["assignee"] =
        json!({"host":"codex","session":"codex-session","agent":"worker","agentType":"builder"});
    body["state"] = json!("active");
    body["id"] = json!(id);
    body["kind"] = json!("task");
    body["stage"] = json!("build");
    assert_written(&yard.write(
        "codex-session",
        &json!([body]).to_string(),
        &["--host", "codex"],
    ));
    let before = read(&yard)["revision"].clone();
    hook(&yard, &parent);
    parent["tool_input"]["plan"] = json!([]);
    hook(&yard, &parent);
    assert_eq!(read(&yard)["revision"], before);
    assert_eq!(
        block(&read(&yard), id)["version"]["body"]["state"],
        "active"
    );
}

fn assert_written(doc: &Value) {
    assert_eq!(doc["outcome"], "written", "{doc}");
}

#[test]
fn a_worker_s_plain_steps_mirror_into_its_working_stage_assigned_to_it() {
    let yard = setup();
    hook(&yard, &plan(&yard, &[("Local checklist", "pending")]));
    let doc = read(&yard);
    let mirrored = doc["stages"][0]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] != "shared")
        .unwrap();
    let body = &mirrored["version"]["body"];
    assert_eq!(body["title"], "Local checklist");
    assert_eq!(body["state"], "open");
    assert_eq!(
        body["assignee"],
        json!({"host":"codex","session":"codex-session","agent":"worker","agentType":"builder"})
    );
    assert_eq!(doc["stages"][0]["owner"]["agent"], Value::Null);
}

#[test]
fn a_worker_whose_assigned_tasks_lie_in_two_stages_mirrors_no_plain_step() {
    let yard = setup();
    assert_written(&yard.write(
        "codex-session",
        r#"[{"id":"check","kind":"stage","title":"Check","status":"planned"},
       {"id":"other","kind":"task","stage":"check","title":"Other scope","state":"open"}]"#,
        &["--host", "codex"],
    ));
    // The worker claims the second stage's task for itself.
    assert_written(&yard.write(
        "codex-session",
        r#"[{"id":"other","kind":"task","stage":"check","title":"Other scope","state":"active",
       "assignee":{"host":"codex","session":"codex-session","agent":"worker","agentType":"builder"}}]"#,
        &["--host", "codex", "--agent", "worker", "--agent-type", "builder"],
    ));
    hook(
        &yard,
        &plan(&yard, &[("[yard:shared] Work", "in_progress")]),
    );
    let before = read(&yard)["revision"].clone();
    hook(
        &yard,
        &plan(
            &yard,
            &[("[yard:shared] Work", "in_progress"), ("Local", "pending")],
        ),
    );
    assert_eq!(read(&yard)["revision"], before);
}

#[test]
fn a_bound_change_applies_to_the_task_as_it_stands_without_a_read_first() {
    let yard = setup();
    hook(
        &yard,
        &plan(&yard, &[("[yard:shared] Work", "in_progress")]),
    );
    // A change made since, which the mirror never read, is kept beside its own.
    let changed = yard.run(&[
        "task",
        "shared",
        "--detail",
        "Changed meanwhile",
        "--host",
        "codex",
        "--session",
        "codex-session",
    ]);
    assert_eq!(document("task", &changed)["outcome"], "changed");
    hook(&yard, &plan(&yard, &[("[yard:shared] Work", "completed")]));
    let after = read(&yard);
    let task = block(&after, "shared");
    assert_eq!(task["version"]["body"]["detail"], "Changed meanwhile");
    assert_eq!(task["version"]["body"]["state"], "done");
}

#[test]
fn a_worker_s_end_returns_all_its_active_tasks_to_open_in_one_write() {
    let yard = setup();
    assert_written(&yard.write(
        "codex-session",
        r#"[
      {"id":"second","kind":"task","stage":"build","title":"Second scope","state":"open",
       "assignee":{"host":"codex","session":"codex-session","agent":"worker","agentType":"builder"}}
    ]"#,
        &["--host", "codex"],
    ));
    hook(
        &yard,
        &plan(
            &yard,
            &[
                ("[yard:shared] Work", "in_progress"),
                ("[yard:second] More", "in_progress"),
            ],
        ),
    );
    let before = read(&yard)["revision"].as_u64().unwrap();
    hook(&yard, &worker(&yard, "SubagentStop"));
    let ended = read(&yard);
    assert_eq!(ended["revision"], before + 1);
    for id in ["shared", "second"] {
        let body = &block(&ended, id)["version"]["body"];
        assert_eq!(body["state"], "open", "{id}");
        assert_eq!(body["assignee"]["agent"], "worker", "{id}");
    }
}

#[test]
fn a_task_assigned_after_the_worker_started_and_made_active_by_hand_is_released_at_its_end() {
    let yard = Yard::new();
    assert_written(&yard.write(
        "codex-session",
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"}]"#,
        &["--host", "codex"],
    ));
    hook(&yard, &worker(&yard, "SubagentStart"));
    assert_written(&yard.write(
        "codex-session",
        r#"[
      {"id":"late","kind":"task","stage":"build","title":"Late scope","state":"open",
       "assignee":{"host":"codex","session":"codex-session","agent":"worker","agentType":"builder"}}
    ]"#,
        &["--host", "codex"],
    ));
    let started = yard.run(&[
        "task",
        "late",
        "--state",
        "active",
        "--host",
        "codex",
        "--session",
        "codex-session",
        "--agent",
        "worker",
        "--agent-type",
        "builder",
    ]);
    assert_eq!(document("task", &started)["outcome"], "changed");
    hook(&yard, &worker(&yard, "SubagentStop"));
    let ended = read(&yard);
    let body = &block(&ended, "late")["version"]["body"];
    assert_eq!(body["state"], "open");
    assert_eq!(body["assignee"]["agent"], "worker");
}

//! Native plan updates are mirrored into only the identified caller's active stage.

use serde_json::{Value, json};

use crate::yard::Yard;
use crate::yard_codex::{event, hook};

fn stage(yard: &Yard) {
    yard.write(
        "codex-session",
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"}]"#,
        &["--host", "codex"],
    );
}

fn plan(yard: &Yard, steps: &[(&str, &str)]) -> Value {
    let mut input = event(yard, "PostToolUse");
    input["tool_name"] = json!("update_plan");
    input["tool_use_id"] = json!("plan-call");
    input["tool_input"] = json!({"plan":steps.iter().map(|(text,status)|json!({"step":text,"status":status})).collect::<Vec<_>>()});
    input["tool_response"] = json!("Plan updated");
    input
}

fn read(yard: &Yard) -> Value {
    yard.read("codex-session", &["--host", "codex"])
}

fn tasks(doc: &Value) -> &[Value] {
    doc["stages"][0]["blocks"].as_array().unwrap()
}

#[test]
fn codex_plan_updates_preserve_task_identity_when_reordered_and_skip_unchanged_versions() {
    let yard = Yard::new();
    stage(&yard);
    hook(
        &yard,
        &plan(&yard, &[("Build", "in_progress"), ("Check", "pending")]),
    );
    let first = read(&yard);
    assert_eq!(tasks(&first).len(), 2, "{first}");
    assert_eq!(tasks(&first)[0]["version"]["body"]["state"], "active");
    assert_eq!(tasks(&first)[1]["version"]["body"]["state"], "open");
    hook(
        &yard,
        &plan(&yard, &[("Check", "pending"), ("Build", "in_progress")]),
    );
    let reordered = read(&yard);
    assert_eq!(first["revision"], reordered["revision"]);
    assert_eq!(tasks(&first)[0]["id"], tasks(&reordered)[0]["id"]);
    hook(
        &yard,
        &plan(&yard, &[("Build", "completed"), ("Check", "in_progress")]),
    );
    let changed = read(&yard);
    assert_eq!(tasks(&changed)[0]["version"]["body"]["state"], "done");
    assert_eq!(tasks(&changed)[1]["version"]["body"]["state"], "active");
    assert!(
        changed.get("lastRead").is_none(),
        "mirroring must not mark a read: {changed}"
    );
}

#[test]
fn renaming_a_codex_step_drops_its_unfinished_task_and_preserves_completed_tasks_and_fixes() {
    let yard = Yard::new();
    stage(&yard);
    hook(
        &yard,
        &plan(
            &yard,
            &[
                ("Done", "completed"),
                ("Rename", "in_progress"),
                ("Fix", "pending"),
            ],
        ),
    );
    let first = read(&yard);
    let mut fix = tasks(&first)[2]["version"]["body"].clone();
    fix["id"] = tasks(&first)[2]["id"].clone();
    fix["kind"] = json!("task");
    fix["stage"] = json!("build");
    fix["type"] = json!("fix");
    yard.write(
        "codex-session",
        &json!([fix]).to_string(),
        &["--host", "codex"],
    );
    hook(
        &yard,
        &plan(&yard, &[("Renamed", "pending"), ("Fix", "in_progress")]),
    );
    let after = read(&yard);
    let blocks = tasks(&after);
    assert_eq!(blocks.len(), 4, "{after}");
    assert_eq!(blocks[0]["version"]["body"]["state"], "done");
    assert_eq!(blocks[1]["version"]["body"]["state"], "dropped");
    assert_eq!(blocks[2]["version"]["body"]["type"], "fix");
    assert_eq!(blocks[2]["version"]["body"]["state"], "active");
    assert_ne!(blocks[1]["id"], blocks[3]["id"]);
}

#[test]
fn duplicate_codex_steps_have_distinct_task_ids_within_the_stage() {
    let yard = Yard::new();
    stage(&yard);
    hook(
        &yard,
        &plan(&yard, &[("Check", "in_progress"), ("Check", "pending")]),
    );
    let doc = read(&yard);
    assert_eq!(tasks(&doc).len(), 2);
    assert_ne!(tasks(&doc)[0]["id"], tasks(&doc)[1]["id"]);
}

#[test]
fn failed_malformed_and_unowned_codex_plans_create_no_task_write() {
    let yard = Yard::new();
    stage(&yard);
    let before = read(&yard)["revision"].clone();
    let mut input = plan(&yard, &[("Check", "pending")]);
    input["tool_response"] = json!({"isError":true});
    hook(&yard, &input);
    hook(&yard, &plan(&yard, &[("Check", "invented")]));
    let mut other = plan(&yard, &[("Check", "pending")]);
    other["session_id"] = json!("another-session");
    hook(&yard, &other);
    assert_eq!(read(&yard)["revision"], before);
    let absent = Yard::new();
    hook(&absent, &plan(&absent, &[("Check", "pending")]));
    assert_eq!(read(&absent)["revision"], 0);
}

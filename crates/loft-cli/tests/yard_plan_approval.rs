//! A session's plan mode under a host that ends `ExitPlanMode` itself, as an Agent SDK host
//! does: the person decides the plan on the page or in the terminal, no `PermissionRequest`
//! answer changes the session's mode, and every hook after the approval still names the
//! permission mode `plan`.

use std::fs;

use serde_json::{Value, json};

use crate::common::EPOCH;
use crate::served::{Served, free_port};
use crate::yard::{BRANCH, Yard};
use crate::yard_claude_hook::{hook, input};
use crate::yard_serve_events::{dir, log, post, session_file};

/// When the hooks and the terminal's answers happen: the fixed clock's time.
const AT: &str = "2026-01-01T00:00:00Z";

/// When the page server records what the page decides, a minute and a half later.
const DECIDED: &str = "2026-01-01T00:01:30Z";

/// The page server's clock, at [`DECIDED`].
fn deciding() -> String {
    (EPOCH + 90).to_string()
}

/// Yard Log with a plan, `p1`, presented in plan mode where `plan_mode` says so and sent by
/// session `s1`'s agent named by `agent`, and a page server.
fn presented(plan_mode: bool, agent: &[&str]) -> (Yard, Served) {
    let yard = Yard::new();
    let plan = json!([
        {"id": "build", "kind": "stage", "title": "Build", "status": "active"},
        {"id": "p1", "kind": "plan", "stage": "build", "title": "The plan",
         "markdown": "1. Do it", "planMode": plan_mode},
    ]);
    let mut args = vec!["--send"];
    args.extend_from_slice(agent);
    yard.write("s1", &plan.to_string(), &args);
    let served = serve(&yard);
    (yard, served)
}

/// A page server for `yard`'s tree, deciding at [`DECIDED`].
fn serve(yard: &Yard) -> Served {
    let tree = yard.scratch.0.display().to_string();
    Served::start_with(
        yard,
        free_port(),
        &[("SOURCE_DATE_EPOCH", &deciding())],
        &["--tree", &tree],
    )
}

/// A hook of `event` of session `s1`'s main agent naming the permission mode `plan`, for the tool
/// `tool` where one is named.
fn planning(yard: &Yard, event: &str, tool: Option<&str>) -> Value {
    let mut planning = input(yard, event, "s1");
    planning["permission_mode"] = json!("plan");
    if let Some(tool) = tool {
        planning["tool_name"] = json!(tool);
    }
    planning
}

/// Decides `p1` on the page.
fn decide(yard: &Yard, served: &Served, outcome: &Value) {
    let sent = json!({"tree": dir(yard), "branch": BRANCH, "answers": [outcome]});
    let reply = served.post_as_page("/api/answer", &sent);
    assert_eq!(reply.status, 200, "{}", reply.json());
}

/// The kinds of the events session `s1`'s event log holds, in order.
fn kinds(yard: &Yard) -> Vec<String> {
    log(yard, "s1")
        .iter()
        .map(|event| event["kind"].as_str().unwrap().to_owned())
        .collect()
}

/// Session `s1`'s activity file.
fn activity(yard: &Yard) -> String {
    session_file(yard, "activity", "s1").unwrap()
}

/// The main agent enters plan mode and presents `p1`, and the host ends `ExitPlanMode` itself.
fn planned(yard: &Yard) {
    hook(
        yard,
        &planning(yard, "PostToolUse", Some("EnterPlanMode")),
        None,
    );
    hook(
        yard,
        &planning(yard, "PostToolUse", Some("ExitPlanMode")),
        None,
    );
}

/// The main agent goes on building while the host still names the mode `plan`.
fn builds(yard: &Yard) {
    hook(yard, &planning(yard, "PostToolUse", Some("Write")), None);
    hook(yard, &planning(yard, "Stop", None), None);
}

/// Asserts that session `s1`'s one stretch of plan mode started at [`AT`] and is going on.
fn assert_planning(yard: &Yard) {
    let activity = activity(yard);
    assert!(
        activity.contains(&format!("plan:\n  - started: {AT}\n")) && !activity.contains("ended:"),
        "{activity}"
    );
    assert_eq!(kinds(yard), ["plan.start"]);
}

#[test]
fn a_plan_mode_plan_approved_on_the_page_ends_its_session_s_plan_span_while_the_host_reports_plan()
{
    let (yard, served) = presented(true, &[]);
    planned(&yard);
    decide(
        &yard,
        &served,
        &json!({"block": "p1", "verdict": "approved"}),
    );
    builds(&yard);
    let activity = activity(&yard);
    assert!(
        activity.contains(&format!("plan:\n  - started: {AT}\n    ended: {DECIDED}\n")),
        "{activity}"
    );
    assert_eq!(kinds(&yard), ["plan.start", "plan.end"]);
}

#[test]
fn a_plan_mode_plan_approved_in_the_terminal_ends_its_session_s_plan_span() {
    let (yard, _served) = presented(true, &[]);
    planned(&yard);
    yard.answer("s1", r#"[{"block": "p1", "verdict": "approved"}]"#);
    builds(&yard);
    let activity = activity(&yard);
    assert!(
        activity.contains(&format!("plan:\n  - started: {AT}\n    ended: {AT}\n")),
        "{activity}"
    );
    assert_eq!(kinds(&yard), ["plan.start", "plan.end"]);
}

#[test]
fn a_plan_rejected_on_the_page_leaves_its_session_s_plan_span_going_on() {
    let (yard, served) = presented(true, &[]);
    planned(&yard);
    decide(
        &yard,
        &served,
        &json!({"block": "p1", "verdict": "rejected", "reason": "Smaller steps"}),
    );
    hook(&yard, &planning(&yard, "PostToolUse", Some("Write")), None);
    assert_planning(&yard);
}

#[test]
fn a_plan_a_subagent_presented_ends_no_plan_span_when_approved() {
    let (yard, served) = presented(true, &["--agent", "planner", "--agent-type", "Plan"]);
    planned(&yard);
    decide(
        &yard,
        &served,
        &json!({"block": "p1", "verdict": "approved"}),
    );
    builds(&yard);
    assert_planning(&yard);
}

#[test]
fn a_plan_presented_outside_plan_mode_ends_no_plan_span_when_approved() {
    let (yard, served) = presented(false, &[]);
    planned(&yard);
    decide(
        &yard,
        &served,
        &json!({"block": "p1", "verdict": "approved"}),
    );
    builds(&yard);
    assert_planning(&yard);
}

#[test]
fn a_page_server_that_rebuilds_the_activity_from_the_event_log_ends_the_plan_span_at_the_approval()
{
    let (yard, served) = presented(true, &[]);
    planned(&yard);
    decide(
        &yard,
        &served,
        &json!({"block": "p1", "verdict": "approved"}),
    );
    drop(served);
    for copy in [yard.live(BRANCH), yard.folder(BRANCH)] {
        let _ = fs::remove_file(copy.join("activity/s1.yaml"));
    }
    let again = serve(&yard);
    let turn_end = json!({"session": "s1", "dir": dir(&yard), "at": "2026-01-01T00:02:00Z", "kind": "turn.end"});
    post(&again, &[turn_end]);
    let activity = activity(&yard);
    assert!(
        activity.contains(&format!("plan:\n  - started: {AT}\n    ended: {DECIDED}\n")),
        "{activity}"
    );
}

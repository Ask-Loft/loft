//! A session's plan mode under a host that ends `ExitPlanMode` itself, as an Agent SDK host
//! does: the person decides the plan on the page or in the terminal, no `PermissionRequest`
//! answer changes the session's mode, and every hook after the approval still names the
//! permission mode `plan`.

use std::fs;

use serde_json::{Value, json};

use crate::common::EPOCH;
use crate::served::{Served, free_port};
use crate::yard::{BRANCH, Yard};
use crate::yard_claude_events::{decide, kinds, moded};
use crate::yard_claude_hook::hook;
use crate::yard_serve_events::{dir, log, post, session_file};

/// When the hooks and the terminal's answers happen: the fixed clock's time.
const AT: &str = "2026-01-01T00:00:00Z";

/// When the page server records what the page decides, a minute and a half later.
const DECIDED: &str = "2026-01-01T00:01:30Z";

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
        &[("SOURCE_DATE_EPOCH", &(EPOCH + 90).to_string())],
        &["--tree", &tree],
    )
}

/// A hook of `event` of session `s1`'s main agent naming the permission mode `plan`, for the tool
/// `tool` where one is named.
fn planning(yard: &Yard, event: &str, tool: Option<&str>) -> Value {
    let mut planning = moded(yard, event, "plan");
    if let Some(tool) = tool {
        planning["tool_name"] = json!(tool);
    }
    planning
}

/// Runs the hook [`planning`] gives.
fn hear(yard: &Yard, event: &str, tool: Option<&str>) {
    hook(yard, &planning(yard, event, tool), None);
}

/// Approves `p1` on the page.
fn approve(yard: &Yard, served: &Served) {
    decide(yard, served, &json!({"block": "p1", "verdict": "approved"}));
}

/// Session `s1`'s activity file.
fn activity(yard: &Yard) -> String {
    session_file(yard, "activity", "s1").unwrap()
}

/// Asserts that session `s1`'s activity file holds the stretches of plan mode `spans`, and what
/// follows them where `spans` names it.
fn assert_plan(yard: &Yard, spans: &str) {
    let activity = activity(yard);
    assert!(activity.contains(&format!("plan:\n{spans}")), "{activity}");
}

/// The main agent enters plan mode and presents `p1`, and the host ends `ExitPlanMode` itself.
fn planned(yard: &Yard) {
    hear(yard, "PostToolUse", Some("EnterPlanMode"));
    hear(yard, "PostToolUse", Some("ExitPlanMode"));
}

/// The main agent goes on building while the host still names the mode `plan`.
fn builds(yard: &Yard) {
    hear(yard, "PostToolUse", Some("Write"));
    hear(yard, "Stop", None);
}

/// Asserts that session `s1`'s one stretch of plan mode started at [`AT`] and is going on.
fn assert_planning(yard: &Yard) {
    assert_plan(yard, &format!("  - started: {AT}\n"));
    assert!(!activity(yard).contains("ended:"), "{}", activity(yard));
    assert_eq!(kinds(yard), ["plan.start"]);
}

/// Asserts that session `s1`'s one stretch of plan mode started at [`AT`] and ended at `ended`.
fn assert_ended(yard: &Yard, ended: &str) {
    assert_plan(yard, &format!("  - started: {AT}\n    ended: {ended}\n"));
}

#[test]
fn a_plan_mode_plan_approved_on_the_page_ends_its_session_s_plan_span_while_the_host_reports_plan()
{
    let (yard, served) = presented(true, &[]);
    planned(&yard);
    approve(&yard, &served);
    builds(&yard);
    assert_ended(&yard, DECIDED);
    assert_eq!(kinds(&yard), ["plan.start", "plan.end"]);
}

#[test]
fn a_plan_mode_plan_approved_in_the_terminal_ends_its_session_s_plan_span() {
    let (yard, _served) = presented(true, &[]);
    planned(&yard);
    yard.answer("s1", r#"[{"block": "p1", "verdict": "approved"}]"#);
    builds(&yard);
    assert_ended(&yard, AT);
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
    hear(&yard, "PostToolUse", Some("Write"));
    assert_planning(&yard);
}

#[test]
fn a_plan_a_subagent_presented_ends_no_plan_span_when_approved() {
    let (yard, served) = presented(true, &["--agent", "planner", "--agent-type", "Plan"]);
    planned(&yard);
    approve(&yard, &served);
    builds(&yard);
    assert_planning(&yard);
}

#[test]
fn a_plan_presented_outside_plan_mode_ends_no_plan_span_when_approved() {
    let (yard, served) = presented(false, &[]);
    planned(&yard);
    approve(&yard, &served);
    builds(&yard);
    assert_planning(&yard);
}

#[test]
fn a_page_server_that_rebuilds_the_activity_from_the_event_log_ends_the_plan_span_at_the_approval()
{
    let (yard, served) = presented(true, &[]);
    planned(&yard);
    approve(&yard, &served);
    drop(served);
    for copy in [yard.live(BRANCH), yard.folder(BRANCH)] {
        let _ = fs::remove_file(copy.join("activity/s1.yaml"));
    }
    let again = serve(&yard);
    let turn_end = json!({"session": "s1", "dir": dir(&yard), "at": "2026-01-01T00:02:00Z", "kind": "turn.end"});
    post(&again, &[turn_end]);
    assert_ended(&yard, DECIDED);
}

#[test]
fn enter_plan_mode_starts_a_new_plan_span_and_ends_the_spec_span_while_the_host_reports_plan() {
    let (yard, served) = presented(true, &[]);
    planned(&yard);
    approve(&yard, &served);
    let spec_edit = json!({
        "session": "s1",
        "dir": dir(&yard),
        "at": AT,
        "kind": "tool.start",
        "tool": "Edit",
        "call": "e1",
        "spec": true,
    });
    post(&served, &[spec_edit]);
    hear(&yard, "PostToolUse", Some("EnterPlanMode"));
    assert_plan(
        &yard,
        &format!(
            "  - started: {AT}\n    ended: {DECIDED}\n  - started: {AT}\nspec:\n  - started: {AT}\n    ended: {AT}\n"
        ),
    );
    assert_eq!(
        kinds(&yard),
        ["plan.start", "plan.end", "tool.start", "plan.start"]
    );
}

#[test]
fn enter_plan_mode_heard_as_the_mode_becomes_plan_posts_plan_start_once() {
    let (yard, _served) = presented(true, &[]);
    hook(&yard, &moded(&yard, "UserPromptSubmit", "default"), None);
    hear(&yard, "PostToolUse", Some("EnterPlanMode"));
    assert_eq!(kinds(&yard), ["plan.start"]);
}

#[test]
fn a_subagent_s_enter_plan_mode_posts_nothing_and_leaves_the_session_s_mode() {
    let (yard, _served) = presented(true, &[]);
    hook(&yard, &moded(&yard, "Stop", "default"), None);
    let mut sub = planning(&yard, "PostToolUse", Some("EnterPlanMode"));
    sub["agent_id"] = json!("worker");
    sub["agent_type"] = json!("Plan");
    hook(&yard, &sub, None);
    assert_eq!(kinds(&yard), Vec::<String>::new());
    assert_eq!(yard.home_file("claude/modes.json"), json!({"s1": false}));
}

/// The text of `~/.yard/<name>`, where it is there.
fn home_text(yard: &Yard, name: &str) -> Option<String> {
    fs::read_to_string(yard.scratch.home().join(".yard").join(name)).ok()
}

#[test]
fn the_approval_s_event_names_the_person_as_who_made_it() {
    let (yard, served) = presented(true, &[]);
    planned(&yard);
    approve(&yard, &served);
    let events = log(&yard, "s1");
    let last = events.last().unwrap();
    assert_eq!(last["kind"], "plan.end", "{last}");
    assert_eq!(last["by"], "person", "{last}");
}

#[test]
fn an_approval_is_not_taken_as_a_sign_that_its_session_is_live() {
    let (yard, served) = presented(true, &[]);
    let before = home_text(&yard, "sessions.json");
    approve(&yard, &served);
    assert_eq!(home_text(&yard, "sessions.json"), before);
    assert_eq!(kinds(&yard), ["plan.end"]);
}

#[test]
fn an_approval_leaves_the_directory_kept_for_its_session_s_main_agent_as_it_was() {
    let (yard, served) = presented(true, &[]);
    let elsewhere = Yard::new();
    let main_dir = dir(&elsewhere);
    let dirs = yard.scratch.home().join(".yard/session-dirs.json");
    fs::write(&dirs, json!({"s1": main_dir}).to_string()).unwrap();
    approve(&yard, &served);
    assert_eq!(yard.home_file("session-dirs.json"), json!({"s1": main_dir}));
    assert_eq!(kinds(&yard), ["plan.end"]);
}

#[test]
fn an_approval_that_is_the_first_event_heard_of_its_session_starts_no_turn() {
    let (yard, served) = presented(true, &[]);
    approve(&yard, &served);
    assert_eq!(kinds(&yard), ["plan.end"]);
    let activity = session_file(&yard, "activity", "s1").unwrap_or_default();
    assert!(!activity.contains("turns:"), "{activity}");
}

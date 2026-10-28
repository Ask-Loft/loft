//! The page's decision of a plan. When the agent presents a plan with `ExitPlanMode`, the Loft
//! mod sends it to Yard Log before Claude Code asks the person, and Claude Code runs the
//! `PermissionRequest` hook beside its own dialog. The hook waits on the newest open submission
//! of the hook's agent that holds a plan-mode `plan` block, for as long as the person takes, and
//! answers Claude Code where the page decided it: an approval ends plan mode and the agent goes
//! on in auto mode, a rejection gives the agent the person's reason. A decision made in the
//! terminal first ends the wait, and the hook answers nothing.

use serde_json::{Value, json};
use yardlog::YardLog;
use yardlog::body::Body;
use yardlog::query::State;
use yardlog_store::home::Home;

use super::hook::Hook;
use crate::cli::{Format, Output};
use crate::yard::args::{Beyond, WaitArgs, Writer};
use crate::yard::branch::{Branch, log_of};
use crate::yard::caller::{Caller, Continuations};
use crate::yard::wait::{LONGEST, attempt};

/// Claude Code's answer to the `PermissionRequest` hook for `ExitPlanMode`: the page's decision,
/// or nothing where there is no plan to wait on or the terminal decided it.
pub fn decide(hook: &Hook, port: u16) -> Value {
    if hook.field("tool_name") != Some("ExitPlanMode") {
        return json!({});
    }
    let Some(submission) = newest(hook, port) else {
        return json!({});
    };
    loop {
        match attempt(hook.dir(), wait_args(hook, &submission, port)) {
            Ok(("pending", _)) => {}
            Ok((_, fields)) => return answer(hook, &fields),
            Err(_) => return json!({}),
        }
    }
}

/// The key of the newest open submission the hook's agent sent that holds a plan-mode plan.
fn newest(hook: &Hook, port: u16) -> Option<String> {
    let branch = Branch::locate(hook.dir(), Beyond { port }).ok()?;
    let held = branch.store.read().ok()?;
    let log = log_of(&held)?;
    let home = Home::from_env();
    let continuations = Continuations::read(home.as_ref()).ok()?;
    let caller = Caller::new(&hook.runner());
    let mine = continuations.identity(caller.agent());
    log.root
        .submissions
        .iter()
        .filter(|submission| submission.state(log) == State::Open)
        .filter(|submission| {
            submission
                .sender()
                .agent()
                .is_some_and(|sender| continuations.identity(sender) == mine)
        })
        .filter(|submission| submission.blocks.iter().any(|key| is_plan(log, *key)))
        .max_by_key(|submission| (submission.sending().at, submission.sending().revision))
        .map(|submission| submission.key.to_string())
}

fn is_plan(log: &YardLog, key: yardlog::BlockKey) -> bool {
    log.block(key).is_some_and(
        |(_, block)| matches!(&block.current().body, Body::Plan(plan) if plan.plan_mode),
    )
}

fn wait_args(hook: &Hook, submission: &str, port: u16) -> WaitArgs {
    let runner = hook.runner();
    WaitArgs {
        submission: submission.to_owned(),
        timeout: LONGEST,
        writer: Writer {
            host: runner.host,
            session: hook.session().to_owned(),
            agent: runner.agent,
            agent_type: runner.agent_type,
        },
        beyond: Beyond { port },
        output: Output {
            format: Some(Format::Json),
        },
    }
}

/// Claude Code's answer from what the wait gave: the plan's outcome, where the page gave it.
fn answer(hook: &Hook, fields: &Value) -> Value {
    let Some(outcome) = fields["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|block| block["kind"] == "plan")
        .map(|block| &block["outcome"])
        .filter(|outcome| outcome["channel"] == "page")
    else {
        return json!({});
    };
    let decision = if outcome["verdict"] == "approved" {
        let input = &hook.raw()["tool_input"];
        let mut kept = serde_json::Map::new();
        for field in ["plan", "planFilePath"] {
            if let Some(text) = input[field].as_str() {
                kept.insert(field.to_owned(), json!(text));
            }
        }
        json!({
            "behavior": "allow",
            "updatedInput": kept,
            "updatedPermissions": [{"type": "setMode", "mode": "auto", "destination": "session"}],
        })
    } else {
        json!({"behavior": "deny", "message": rejection(outcome["reason"].as_str())})
    };
    json!({"hookSpecificOutput": {"hookEventName": hook.event(), "decision": decision}})
}

/// What the agent is told of a rejection on the page, as the Loft mod words it.
fn rejection(reason: Option<&str>) -> String {
    match reason.filter(|reason| !reason.is_empty()) {
        Some(reason) => format!("The person rejected the plan on the Yard page: {reason}"),
        None => "The person rejected the plan on the Yard page.".to_owned(),
    }
}

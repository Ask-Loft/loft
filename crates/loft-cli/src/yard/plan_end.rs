//! The end of a session's plan mode that the person's approval makes (the decision "A plan's
//! approval ends its session's plan mode"): a host that ends `ExitPlanMode` itself may go on
//! reporting the mode `plan` after the person approved the plan on the page, so the approval of
//! a plan a session's main agent presented in plan mode ends that session's stretch of plan mode
//! by an event of its own, kept in the session's event log like every other.

use std::path::Path;

use serde_json::Value;
use yardlog::body::{Body, Verdict};
use yardlog::log::Given;
use yardlog::query::State;
use yardlog::respond::{Answered, Reply};
use yardlog::{Agent, BlockKey, Time, YardLog};

use super::caller::Continuations;
use super::event::main_event;

/// The `plan.end` event, at `at`, of each session whose main agent presented in plan mode a plan
/// that `replies` approved and `answered` recorded, and of the latest session that
/// `continuations` record as continuing it, in the tree `tree`: none for a plan decided
/// elsewhere first, rejected, presented by a subagent or presented outside plan mode.
pub fn events(
    log: &YardLog,
    (replies, answered): (&[Reply], &Answered),
    continuations: Option<&Continuations>,
    at: Time,
    tree: &Path,
) -> Vec<Value> {
    let dir = tree.display().to_string();
    replies
        .iter()
        .filter(|reply| {
            let approved = matches!(&reply.given, Given::Outcome { verdict, .. } if *verdict == Verdict::Approved);
            approved && answered.recorded.contains(&reply.block) && in_plan_mode(log, reply.block)
        })
        .filter_map(|reply| presenter(log, reply.block).filter(|agent| agent.subagent.is_none()))
        .flat_map(|agent| {
            let latest = continuations.and_then(|known| latest(known, agent));
            let sessions = std::iter::once(agent.session.clone()).chain(latest);
            sessions.map(|session| {
                let mut event = main_event(&agent.host, &session, &dir, "plan.end", at);
                event["by"] = "person".into();
                event
            })
        })
        .collect()
}

/// The latest session that continues `agent`'s, where one does: the end of its chain of
/// continuations, which a chain that comes back on itself ends where it would repeat.
fn latest(continuations: &Continuations, agent: &Agent) -> Option<String> {
    let mut seen = vec![agent.session.clone()];
    while let Some(next) = continuations.continued_by(&agent.host, seen.last()?) {
        if seen.contains(&next) {
            break;
        }
        seen.push(next);
    }
    seen.pop().filter(|last| *last != agent.session)
}

/// Whether `block` is a plan presented in plan mode.
pub fn in_plan_mode(log: &YardLog, block: BlockKey) -> bool {
    log.block(block).is_some_and(
        |(_, block)| matches!(&block.current().body, Body::Plan(plan) if plan.plan_mode),
    )
}

/// The agent that sent the latest submission of `block` not withdrawn, where an agent sent it.
fn presenter(log: &YardLog, block: BlockKey) -> Option<&Agent> {
    log.root
        .submissions
        .iter()
        .filter(|submission| submission.blocks.contains(&block))
        .filter(|submission| submission.state(log) != State::Withdrawn)
        .max_by_key(|submission| (submission.sending().at, submission.sending().revision))
        .and_then(|submission| submission.sender().agent())
}

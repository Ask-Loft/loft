//! Live connections are disposable; queued counters and checkpoints are durable.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use loft_ports::clock::Clock as _;

use super::app_server::{Connection, Notice};
use super::state::{Files, State};
use super::usage::{Active, Source};
use super::{Result, usage};

/// What a step's connection attempt and poll found, outside the state lock.
struct Outcome {
    attempted: Option<bool>,
    polled: Option<Result<Vec<Notice>>>,
}

#[derive(Default)]
pub struct Observer {
    connections: BTreeMap<String, (String, Connection)>,
    retry: BTreeMap<String, Duration>,
}

impl Observer {
    /// Steps each active turn of the state the tick read, and tells whether a step changed the
    /// accounting.
    pub fn tick(
        &mut self,
        files: &Files,
        active: &BTreeMap<String, Active>,
        path: Option<&Path>,
        clock: &impl loft_ports::clock::Clock,
    ) -> Result<bool> {
        self.connections
            .retain(|thread, (turn, _)| active.get(thread).is_some_and(|a| a.turn == *turn));
        self.retry.retain(|thread, _| active.contains_key(thread));
        let mut changed = false;
        for (thread, turn) in active {
            changed |= self.step(files, thread, turn, path, clock.elapsed())?;
        }
        Ok(changed)
    }

    fn attempt(
        &mut self,
        thread: &str,
        turn: &Active,
        path: Option<&Path>,
        now: Duration,
    ) -> Option<bool> {
        if turn.ended
            || self.connections.contains_key(thread)
            || self.retry.get(thread).is_some_and(|next| now < *next)
        {
            return None;
        }
        let connection = path.and_then(|path| Connection::connect(path, thread).ok());
        let ready = connection.is_some();
        if let Some(connection) = connection {
            self.connections
                .insert(thread.into(), (turn.turn.clone(), connection));
        }
        self.retry
            .insert(thread.into(), now + Duration::from_secs(5));
        Some(ready)
    }

    fn step(
        &mut self,
        files: &Files,
        thread: &str,
        turn: &Active,
        path: Option<&Path>,
        now: Duration,
    ) -> Result<bool> {
        let at = crate::yard::now().map_err(super::message)?.to_string();
        let attempted = self.attempt(thread, turn, path, now);
        let polled = self.connections.get_mut(thread).map(|(_, c)| c.poll());
        if attempted.is_none() && polled.is_none() && !turn.ended {
            return Ok(false);
        }
        files.update(|state| {
            Ok(self.settle(state, thread, turn, Outcome { attempted, polled }, &at))
        })
    }

    /// Applies one turn's connection outcome to the state, and tells whether the accounting
    /// changed.
    fn settle(
        &mut self,
        state: &mut State,
        thread: &str,
        turn: &Active,
        outcome: Outcome,
        at: &str,
    ) -> bool {
        if state
            .usage
            .active
            .get(thread)
            .is_none_or(|a| a.turn != turn.turn)
        {
            return false;
        }
        let before = state.usage.clone();
        if let Some(ready) = outcome.attempted {
            source(state, thread, turn.source, ready, at);
        }
        match outcome.polled {
            Some(Ok(notices)) => apply(state, thread, &turn.turn, notices, at),
            Some(Err(_)) => {
                self.connections.remove(thread);
                source(state, thread, turn.source, false, at);
            }
            None => {}
        }
        if let Some(source) = state
            .usage
            .active
            .get(thread)
            .filter(|a| a.ended)
            .map(|a| a.source)
        {
            if source == Source::Pending {
                usage::gap(state, thread, at);
            }
            state.usage.active.remove(thread);
            self.connections.remove(thread);
            self.retry.remove(thread);
        }
        state.usage != before
    }
}

fn source(state: &mut State, thread: &str, previous: Source, ready: bool, at: &str) {
    if previous != Source::Pending || !ready {
        usage::gap(state, thread, at);
    }
    if let Some(active) = state.usage.active.get_mut(thread) {
        active.source = if ready {
            Source::Available
        } else {
            Source::Unavailable
        };
    }
}

fn apply(state: &mut State, subscribed: &str, active_turn: &str, notices: Vec<Notice>, at: &str) {
    for notice in notices {
        match notice {
            Notice::Usage {
                thread,
                turn,
                usage,
            } if thread == subscribed && turn == active_turn => {
                usage::observe(state, &thread, &turn, &usage, at);
            }
            Notice::End { thread, turn } if thread == subscribed && turn == active_turn => {
                if let Some(active) = state.usage.active.get_mut(&thread) {
                    active.ended = true;
                }
            }
            _ => {}
        }
    }
}

/// How often the prompt hook looks for the worker's readiness, and how long it looks.
const READINESS_POLL: Duration = Duration::from_millis(50);
const READINESS_BOUND: Duration = Duration::from_millis(500);

/// The prompt hook never waits for delivery, and never waits while holding the state lock.
pub fn readiness(files: &Files, hook: &super::translate::Hook) {
    if hook.kind() != super::translate::HookKind::UserPromptSubmit {
        return;
    }
    let thread = hook.thread();
    let turn = hook.0["turn_id"].as_str();
    let clock = crate::yard::clock();
    let deadline = clock.elapsed() + READINESS_BOUND;
    while clock.elapsed() < deadline {
        if files.read().is_ok_and(|s| {
            let usage_ready = s
                .usage
                .active
                .get(thread)
                .is_none_or(|a| Some(a.turn.as_str()) != turn || a.source != Source::Pending);
            let notification_ready = s.notifications.callers.get(thread).is_none_or(|w| {
                !w.armed || w.availability != super::notification_state::Availability::Pending
            });
            usage_ready && notification_ready
        }) {
            break;
        }
        clock.wait(READINESS_POLL.min(deadline.saturating_sub(clock.elapsed())));
    }
}

#[cfg(test)]
mod tests {
    use loft_ports::clock::SystemClock;
    use serde_json::json;

    use super::*;

    #[test]
    fn accounting_reconnection_backoff_uses_the_passing_clock() {
        let clock = SystemClock::with_epoch(Some("1000")).passing_if(true);
        let active: Active = serde_json::from_value(json!({
            "session":"session","agent":null,"turn":"turn","dir":"/work",
            "model":"fixture","ended":false,"source":"pending","gap":false
        }))
        .unwrap();
        let mut observer = Observer::default();
        assert_eq!(
            observer.attempt("session", &active, None, clock.elapsed()),
            Some(false)
        );
        clock.wait(Duration::from_secs(4));
        assert_eq!(
            observer.attempt("session", &active, None, clock.elapsed()),
            None
        );
        clock.wait(Duration::from_secs(1));
        assert_eq!(
            observer.attempt("session", &active, None, clock.elapsed()),
            Some(false)
        );
    }
}

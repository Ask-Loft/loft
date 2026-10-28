//! Live connections are disposable; queued counters and checkpoints are durable.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use super::app_server::{Connection, Notice};
use super::state::{Files, State};
use super::usage::{Active, Source};
use super::{Result, usage};

#[derive(Default)]
pub struct Observer {
    connections: BTreeMap<String, (String, Connection)>,
    retry: BTreeMap<String, Duration>,
}

impl Observer {
    pub fn tick(
        &mut self,
        files: &Files,
        path: Option<&Path>,
        clock: &impl loft_ports::clock::Clock,
    ) -> Result<()> {
        let active = {
            let _locked = files.lock()?;
            files.read()?.usage.active
        };
        self.connections
            .retain(|thread, (turn, _)| active.get(thread).is_some_and(|a| a.turn == *turn));
        self.retry.retain(|thread, _| active.contains_key(thread));
        for (thread, turn) in active {
            self.step(files, &thread, &turn, path, clock.elapsed())?;
        }
        Ok(())
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
    ) -> Result<()> {
        let at = crate::yard::now().map_err(super::message)?.to_string();
        let attempted = self.attempt(thread, turn, path, now);
        let polled = self.connections.get_mut(thread).map(|(_, c)| c.poll());
        let _locked = files.lock()?;
        let mut state = files.read()?;
        if state
            .usage
            .active
            .get(thread)
            .is_none_or(|a| a.turn != turn.turn)
        {
            return Ok(());
        }
        let before = state.usage.clone();
        if let Some(ready) = attempted {
            source(&mut state, thread, turn.source, ready, &at);
        }
        if let Some(result) = polled {
            if let Ok(notices) = result {
                apply(&mut state, thread, &turn.turn, notices, &at);
            } else {
                self.connections.remove(thread);
                source(&mut state, thread, turn.source, false, &at);
            }
        }
        if state.usage.active.get(thread).is_some_and(|a| a.ended) {
            if state
                .usage
                .active
                .get(thread)
                .is_some_and(|a| a.source == Source::Pending)
            {
                usage::gap(&mut state, thread, &at);
            }
            state.usage.active.remove(thread);
            self.connections.remove(thread);
            self.retry.remove(thread);
        }
        if state.usage != before {
            files.write(&state)?;
        }
        Ok(())
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

/// The prompt hook never waits for delivery, and never waits while holding the state lock.
pub fn readiness(files: &Files, hook: &super::translate::Hook) {
    if hook.name() != "UserPromptSubmit" {
        return;
    }
    let thread = hook.thread();
    let turn = hook.0["turn_id"].as_str();
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
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
        std::thread::sleep(
            Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

#[cfg(test)]
mod tests {
    use loft_ports::clock::{Clock as _, SystemClock};
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

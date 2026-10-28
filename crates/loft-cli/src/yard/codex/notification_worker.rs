//! Worker-owned decision discovery and delivery. Every socket call runs outside the state lock.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use loft_ports::clock::Clock;
use serde_json::json;

use super::Result;
use super::app_server::{CallFailure, notifications::Client};
use super::commands;
use super::notification_discovery::Snapshot;
use super::notification_state::{Attempt, Availability, Ledger, Progress, Watcher};
use super::state::Files;

#[derive(Default)]
pub struct Monitor {
    next: BTreeMap<String, Duration>,
}

impl Monitor {
    /// Refreshes each armed caller of the ledger the tick read.
    pub fn tick(
        &mut self,
        files: &Files,
        ledger: &Ledger,
        socket: Option<&Path>,
        port: u16,
        clock: &impl Clock,
    ) -> Result<()> {
        for watcher in ledger.callers.values().filter(|w| w.armed) {
            let thread = &watcher.route.thread;
            if watcher.availability != Availability::Pending
                && self
                    .next
                    .get(thread)
                    .is_some_and(|at| clock.elapsed() < *at)
            {
                continue;
            }
            self.next
                .insert(thread.clone(), clock.elapsed() + Duration::from_secs(5));
            let now = clock.now().map_err(|e| e.to_string())?;
            refresh(files, watcher, socket, port, now)?;
        }
        Ok(())
    }
}

fn refresh(
    files: &Files,
    watcher: &Watcher,
    socket: Option<&Path>,
    port: u16,
    now: u64,
) -> Result<()> {
    let Ok(doc) = commands::history(&watcher.route.directory, &watcher.route.caller, port) else {
        return report(files, watcher, Availability::Unavailable);
    };
    let snapshot = Snapshot::read(&doc, &watcher.route);
    if doc["schema"] != crate::yard::read::SCHEMA
        || (snapshot.is_none() && doc["revision"] != 0)
        || doc["detached"] == true
    {
        return report(files, watcher, Availability::Unavailable);
    }
    let candidates = discover(files, watcher, snapshot.as_ref())?;
    if snapshot.as_ref().is_some_and(|s| s.closed) {
        return report(files, watcher, Availability::Unavailable);
    }
    let Some(path) = socket else {
        return report(files, watcher, Availability::Unavailable);
    };
    let Ok(mut client) = Client::connect(path, &watcher.route.thread) else {
        return report(files, watcher, Availability::Unavailable);
    };
    if client.inspect(Path::new(&watcher.route.directory)).is_err() {
        return report(files, watcher, Availability::Unavailable);
    }
    report(files, watcher, Availability::Available)?;
    for marker in candidates {
        // The uncertainty checkpoint reaches disk before any model input is attempted.
        let attempt = files.update(|state| Ok(state.notifications.claim(&marker, now)))?;
        if let Some(attempt) = attempt {
            dispatch(files, &mut client, &attempt, port)?;
        }
    }
    Ok(())
}

fn discover(files: &Files, watcher: &Watcher, snapshot: Option<&Snapshot>) -> Result<Vec<String>> {
    files.update(|state| {
        let ledger = &mut state.notifications;
        if !live(ledger, watcher) {
            return Ok(vec![]);
        }
        let Some(snapshot) = snapshot else {
            return Ok(vec![]);
        };
        if snapshot.closed {
            ledger.cancel_log(&snapshot.log);
            return Ok(vec![]);
        }
        for marker in &snapshot.withdrawn {
            ledger.cancel_marker(marker);
        }
        Ok(snapshot
            .resolved
            .iter()
            .filter_map(|resolution| ledger.offer(resolution.clone(), snapshot.own_read))
            .collect())
    })
}

fn dispatch(files: &Files, client: &mut Client, attempt: &Attempt, port: u16) -> Result<()> {
    let Some(snapshot) = current_snapshot(attempt, port) else {
        return finish(files, attempt, Err(CallFailure::Unavailable));
    };
    let current = files.update(|state| {
        let ledger = &mut state.notifications;
        if snapshot.contains(&attempt.resolution) {
            ledger.offer(attempt.resolution.clone(), snapshot.own_read);
        } else if snapshot.log == attempt.resolution.log
            && (snapshot.closed || snapshot.withdrawn.contains(&attempt.marker))
        {
            ledger.cancel_marker(&attempt.marker);
        } else {
            ledger.refuse(attempt);
        }
        Ok(ledger.current(attempt))
    })?;
    if !current {
        return Ok(());
    }
    if attempt.needs_receipt {
        match client.received(&attempt.marker) {
            Ok(true) => return finish(files, attempt, Ok(())),
            Ok(false) => {}
            Err(error) => return finish(files, attempt, Err(error)),
        }
    }
    let active = match client.inspect(Path::new(&attempt.route.directory)) {
        Ok(active) => active,
        Err(error) => return finish(files, attempt, Err(error)),
    };
    // Cancellation may have arrived during a native metadata or receipt request.
    let current = files.peek(|state| state.notifications.current(attempt))?;
    if !current {
        return Ok(());
    }
    let result = client.deliver(&attempt.marker, &message(attempt), active.as_deref());
    finish(files, attempt, result.map(|_| ()))
}

fn current_snapshot(attempt: &Attempt, port: u16) -> Option<Snapshot> {
    let doc = commands::history(&attempt.route.directory, &attempt.route.caller, port).ok()?;
    Snapshot::read(&doc, &attempt.route)
}

fn finish(
    files: &Files,
    attempt: &Attempt,
    result: std::result::Result<(), CallFailure>,
) -> Result<()> {
    files.update(|state| {
        let ledger = &mut state.notifications;
        let availability = match result {
            Ok(()) => {
                ledger.confirm(attempt);
                Availability::Available
            }
            Err(CallFailure::Refused) => {
                ledger.refuse(attempt);
                Availability::Available
            }
            Err(CallFailure::Unavailable) => Availability::Unavailable,
        };
        let accepted = ledger
            .deliveries
            .get(&attempt.marker)
            .is_some_and(|t| matches!(t.progress, Progress::Accepted | Progress::Read));
        if ledger.live(&attempt.route.thread, attempt.generation, &attempt.route) {
            ledger.report(
                &attempt.route.thread,
                if accepted {
                    Availability::Available
                } else {
                    availability
                },
            );
        }
        Ok(())
    })
}

fn live(ledger: &Ledger, watcher: &Watcher) -> bool {
    ledger.live(&watcher.route.thread, watcher.generation, &watcher.route)
}

fn report(files: &Files, watcher: &Watcher, availability: Availability) -> Result<()> {
    files.update(|state| {
        if live(&state.notifications, watcher) {
            state
                .notifications
                .report(&watcher.route.thread, availability);
        }
        Ok(())
    })
}

fn message(attempt: &Attempt) -> String {
    let source = &attempt.resolution;
    let metadata = json!({"yardLog":source.log,"tree":source.tree,"branch":source.branch,"submission":source.submission,"revision":source.revision});
    format!(
        "{}\nYard has a resolved submission: {metadata}\nRead authoritative Yard state with loft yard read {} --mark-read --format json in the named tree before deciding how to continue. This adapter notification is not a new approval; a duplicate notification requires a fresh Yard read.",
        attempt.marker,
        attempt.route.caller.args().join(" ")
    )
}

#[cfg(test)]
mod tests;

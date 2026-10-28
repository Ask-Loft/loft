//! A native background hook owns liveness, delivery and its page-server child.

use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use yardlog_store::home::Home;
use yardlog_store::lock;

use crate::version::{VERSION, Version};

use super::Result;
use super::classify_event;
use super::commands;
use super::state::Files;
use super::translate::{Hook, HookKind};
use crate::yard::post::post;

const BEAT: Duration = Duration::from_secs(5);
const LIFETIME: Duration = Duration::from_hours(24);

struct ServerChild(Option<Child>);

impl Drop for ServerChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn run(hook: &Hook, port: u16, socket: Option<&PathBuf>) -> Result<Value> {
    let files = Files::of(hook)?;
    let Some(_worker) = lock::try_lock(&files.0.join("worker.lock")).map_err(|e| e.to_string())?
    else {
        return Ok(json!({}));
    };
    resume(&files, hook)?;
    let home = Home::from_env().ok_or("no machine home for Yard")?;
    let clock = crate::yard::clock();
    let mut server = ServerChild(None);
    let mut observer = super::observer::Observer::default();
    let mut notifications = super::notification_worker::Monitor::default();
    // Learned on each beat; looked up between beats only while no page server is known or
    // the known one refused a post.
    let mut page_port = None;
    drive(clock, |beat| {
        let state = files.peek(|state| state)?;
        let accounted = observer.tick(
            &files,
            &state.usage.active,
            socket.map(PathBuf::as_path),
            clock,
        )?;
        if !state.stopped {
            notifications.tick(
                &files,
                &state.notifications,
                socket.map(PathBuf::as_path),
                port,
                clock,
            )?;
        }
        if beat && !state.stopped {
            let running = crate::yard::page::running(&home).map_err(|e| e.to_string())?;
            page_port = running.as_ref().map(|running| running.port);
            let current_port = page_port.unwrap_or(port);
            let live = running
                .as_ref()
                .is_some_and(|running| Version::parse(&running.version) >= Version::parse(VERSION));
            if !live {
                start_server(hook, port, &mut server)?;
            }
            let _ = post(
                &home,
                current_port,
                "/api/heard",
                &json!({"host":"codex","session":hook.session()}),
            );
            let _ = post(
                &home,
                current_port,
                "/api/register",
                &json!({"tree":hook.dir()}),
            );
        }
        let queued = !state.events.is_empty() || !state.in_flight.is_empty();
        if queued || accounted || state.stopped {
            deliver(&files, &home, &mut page_port)?;
        }
        Ok(!state.stopped)
    })?;
    Ok(json!({}))
}

// Concurrent background startup may precede the ordinary prompt handler. A cancelled nudge
// cannot clear a previous session end, even when Codex launches both handlers at once.
fn resume(files: &Files, hook: &Hook) -> Result<()> {
    if matches!(
        hook.kind(),
        HookKind::SessionStart | HookKind::UserPromptSubmit
    ) {
        files.update(|state| {
            let generated = state
                .notifications
                .is_notification(hook.thread(), hook.0["prompt"].as_str().unwrap_or_default());
            if !generated {
                state.stopped = false;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn drive(
    clock: &impl loft_ports::clock::Clock,
    mut tick: impl FnMut(bool) -> Result<bool>,
) -> Result<()> {
    let began = clock.elapsed();
    let mut until_beat = began;
    while clock.elapsed().saturating_sub(began) < LIFETIME {
        let beat = clock.elapsed() >= until_beat;
        if !tick(beat)? {
            break;
        }
        if beat {
            until_beat = clock.elapsed() + BEAT;
        }
        clock.wait(Duration::from_millis(100));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

fn start_server(hook: &Hook, port: u16, child: &mut ServerChild) -> Result<()> {
    if let Some(running) = &mut child.0
        && running.try_wait().map_err(|e| e.to_string())?.is_none()
    {
        return Ok(());
    }
    let args = [
        "serve",
        "--host",
        "codex",
        "--session",
        hook.session(),
        "--tree",
        hook.dir(),
    ]
    .map(str::to_owned);
    child.0 = Some(
        commands::command(hook.dir(), &args, port)?
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?,
    );
    Ok(())
}

/// Flushes the outbox to the known page server, looking it up first where none is known.
fn deliver(files: &Files, home: &Home, page_port: &mut Option<u16>) -> Result<()> {
    if page_port.is_none() {
        *page_port = crate::yard::page::running(home)
            .ok()
            .flatten()
            .map(|running| running.port);
    }
    let Some(known) = *page_port else {
        return Ok(());
    };
    let mut accepted = true;
    flush(files, |body| {
        accepted = post(home, known, "/api/events", body);
        accepted
    })?;
    // A refused post looks the server up again on the next tick.
    if !accepted {
        *page_port = None;
    }
    Ok(())
}

/// Detaches a persisted batch while posting, so new hooks append to a separate queue. A crash
/// leaves that batch available for replay; a failed post restores it ahead of newer events.
/// Each queued request is classified by the adapter's rule as the batch is detached, so that one
/// queued before cache writes were classified is posted as the adapter now gives it, and the
/// stored batch is posted again byte for byte. Where the page server holds one in its earlier
/// form, it rejects the classified one and counts it once.
fn flush(files: &Files, post: impl FnOnce(&Value) -> bool) -> Result<()> {
    let detached = files.update(|state| {
        for event in state.in_flight.iter_mut().chain(&mut state.events) {
            classify_event(event);
        }
        if state.in_flight.is_empty() {
            if state.events.is_empty() {
                return Ok(None);
            }
            state.in_flight = std::mem::take(&mut state.events);
            state.in_flight_dropped = std::mem::take(&mut state.dropped);
            if state.in_flight_dropped > 0 {
                state.in_flight[0]["dropped"] = json!(state.in_flight_dropped);
            }
        }
        Ok(Some(state.in_flight.clone()))
    })?;
    let Some(body) = detached else {
        return Ok(());
    };
    let accepted = post(&json!(body));
    files.update(|state| {
        // Identified payloads must survive a lost acknowledgement byte-for-byte, including
        // their drop report. Later hooks keep appending to the separate pending queue.
        if !accepted
            && state
                .in_flight
                .iter()
                .any(|event| event.get("eventId").is_some())
        {
            return Ok(());
        }
        if !accepted {
            let mut restored = std::mem::take(&mut state.in_flight);
            restored.append(&mut state.events);
            state.events = restored;
            state.dropped += state.in_flight_dropped;
            state.trim();
        }
        state.in_flight.clear();
        state.in_flight_dropped = 0;
        Ok(())
    })
}

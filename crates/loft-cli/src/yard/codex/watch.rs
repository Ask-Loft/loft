//! A native background hook owns liveness, delivery and its page-server child.

use std::process::{Child, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::lock;

use super::Result;
use super::commands;
use super::state::Files;
use super::translate::Hook;

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

pub fn run(hook: &Hook, port: u16) -> Result<Value> {
    let files = Files::of(hook)?;
    let Some(_worker) = lock::try_lock(&files.0.join("worker.lock")).map_err(|e| e.to_string())?
    else {
        return Ok(json!({}));
    };
    let home = Home::from_env().ok_or("no machine home for Yard")?;
    let clock = crate::yard::clock();
    let mut server = ServerChild(None);
    drive(clock, |beat| {
        {
            let _locked = files.lock()?;
            if files.read()?.stopped {
                return Ok(false);
            }
        }
        if beat {
            let running = crate::yard::page::running(&home).map_err(|e| e.to_string())?;
            let current_port = running.as_ref().map_or(port, |running| running.port);
            let live = running.as_ref().is_some_and(|running| {
                crate::update_check::parse(&running.version)
                    >= crate::update_check::parse(crate::version::VERSION)
            });
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
        if let Ok(Some(running)) = crate::yard::page::running(&home) {
            flush(&files, |body| {
                post(&home, running.port, "/api/events", body)
            })?;
        }
        Ok(true)
    })?;
    Ok(json!({}))
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
#[path = "watch_tests.rs"]
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

fn post(home: &Home, port: u16, path: &str, body: &Value) -> bool {
    let Ok(Some(token)) = home.read(HomeFile::Token) else {
        return false;
    };
    let token = String::from_utf8_lossy(&token);
    crate::network::post_local_json(port, path, token.trim(), body.to_string().as_bytes())
}

/// Detaches a persisted batch while posting, so new hooks append to a separate queue. A crash
/// leaves that batch available for replay; a failed post restores it ahead of newer events.
fn flush(files: &Files, post: impl FnOnce(&Value) -> bool) -> Result<()> {
    let (sent, dropped) = {
        let _locked = files.lock()?;
        let mut state = files.read()?;
        if state.in_flight.is_empty() {
            if state.events.is_empty() {
                return Ok(());
            }
            state.in_flight = std::mem::take(&mut state.events);
            state.in_flight_dropped = std::mem::take(&mut state.dropped);
            files.write(&state)?;
        }
        (state.in_flight, state.in_flight_dropped)
    };
    let mut body = sent;
    if dropped > 0 {
        body[0]["dropped"] = json!(dropped);
    }
    let accepted = post(&json!(body));
    let _locked = files.lock()?;
    let mut state = files.read()?;
    if !accepted {
        let mut restored = std::mem::take(&mut state.in_flight);
        restored.append(&mut state.events);
        state.events = restored;
        state.dropped += state.in_flight_dropped;
        state.trim();
    }
    state.in_flight.clear();
    state.in_flight_dropped = 0;
    files.write(&state)
}

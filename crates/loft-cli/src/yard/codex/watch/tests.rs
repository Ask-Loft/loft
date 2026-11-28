//! Worker scheduling and acknowledgment without a network or a native host.

use loft_ports::clock::{Clock as _, SystemClock};
use serde_json::json;

use super::{drive, flush, serve_args};
use crate::yard::codex::state::{Files, State};

#[test]
fn a_page_server_the_codex_worker_starts_joins_one_of_its_version_and_names_no_address() {
    let args = serve_args("s1", "/work");
    assert!(args.iter().any(|arg| arg == "--join"), "{args:?}");
    assert!(!args.iter().any(|arg| arg == "--listen"), "{args:?}");
}

#[test]
fn the_codex_worker_beats_every_five_seconds_and_ends_after_twenty_four_hours() {
    let clock = SystemClock::with_epoch(Some("0")).passing_if(true);
    let mut beats = 0;
    let mut last = None;
    drive(&clock, |beat| {
        if beat {
            if let Some(last) = last {
                assert_eq!(
                    clock.elapsed().checked_sub(last).unwrap(),
                    std::time::Duration::from_secs(5)
                );
            }
            last = Some(clock.elapsed());
            beats += 1;
        }
        Ok(true)
    })
    .unwrap();
    assert_eq!(clock.elapsed(), std::time::Duration::from_hours(24));
    assert_eq!(beats, 17_280);
}

#[test]
fn acknowledging_a_codex_post_keeps_events_queued_during_delivery_and_reports_drops() {
    let root = std::env::temp_dir().join(format!("codex-outbox-{}", uuid::Uuid::new_v4()));
    let files = Files(root.clone());
    let mut state = State::default();
    state.keep((0..1002).map(|n| json!({"n":n})).collect());
    files.write(&state).unwrap();
    flush(&files, |body| {
        assert_eq!(body[0]["dropped"], 2);
        assert_eq!(body[0]["n"], 2);
        assert_eq!(body[999]["n"], 1001);
        assert!(body[0].get("queueId").is_none());
        let _locked = files.lock().unwrap();
        let mut during = files.read().unwrap();
        during.keep(vec![json!({"n":1002})]);
        files.write(&during).unwrap();
        true
    })
    .unwrap();
    let left = files.read().unwrap();
    assert_eq!(left.events.len(), 1);
    assert_eq!(left.events[0]["n"], 1002);
    assert_eq!(left.dropped, 0);
    flush(&files, |_| false).unwrap();
    assert_eq!(files.read().unwrap().events, left.events);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_failed_codex_delivery_restores_its_batch_before_new_events_for_retry() {
    let root = std::env::temp_dir().join(format!("codex-retry-{}", uuid::Uuid::new_v4()));
    let files = Files(root.clone());
    let mut state = State::default();
    state.keep(vec![json!({"n":1})]);
    files.write(&state).unwrap();
    flush(&files, |_| {
        let _locked = files.lock().unwrap();
        let mut state = files.read().unwrap();
        state.keep(vec![json!({"n":2})]);
        files.write(&state).unwrap();
        false
    })
    .unwrap();
    assert_eq!(
        files.read().unwrap().events,
        [json!({"n":1}), json!({"n":2})]
    );
    flush(&files, |body| {
        assert_eq!(body, &json!([{"n":1},{"n":2}]));
        true
    })
    .unwrap();
    assert_eq!(
        files.read().unwrap().events,
        Vec::<serde_json::Value>::new()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_identified_in_flight_payload_and_drop_report_survive_failure_and_restart_unchanged() {
    let root = std::env::temp_dir().join(format!("codex-identified-{}", uuid::Uuid::new_v4()));
    let files = Files(root.clone());
    let mut state = State::default();
    state.keep(vec![
        json!({"eventId":"observation","at":"original","tokens":{"input":7}}),
    ]);
    state.dropped = 3;
    files.write(&state).unwrap();
    let mut original = serde_json::Value::Null;
    flush(&files, |body| {
        original = body.clone();
        let mut during = files.read().unwrap();
        during.keep((0..1002).map(|n| json!({"n":n})).collect());
        files.write(&during).unwrap();
        false
    })
    .unwrap();
    assert_eq!(files.read().unwrap().in_flight[0]["dropped"], 3);
    let restarted = Files(root.clone());
    flush(&restarted, |body| {
        assert_eq!(body, &original);
        true
    })
    .unwrap();
    assert_eq!(restarted.read().unwrap().events.len(), 1000);
    assert_eq!(restarted.read().unwrap().dropped, 2);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_background_handler_for_a_canceled_nudge_cannot_resume_an_ended_session() {
    use crate::yard::codex::commands::Caller;
    use crate::yard::codex::notification_state::{Lifecycle, Resolution, Route};
    use crate::yard::codex::translate::Hook;

    let root = std::env::temp_dir().join(format!("codex-ended-nudge-{}", uuid::Uuid::new_v4()));
    let files = Files(root.clone());
    let route = Route {
        caller: Caller {
            session: "session".into(),
            agent: None,
        },
        thread: "session".into(),
        directory: "/work".into(),
    };
    let mut state = State::default();
    state.notifications.observe(route.clone(), Lifecycle::Start);
    let marker = state
        .notifications
        .offer(
            Resolution {
                log: "log".into(),
                submission: "submission".into(),
                revision: 2,
                tree: "/work".into(),
                branch: "feat/shelf".into(),
                thread: "session".into(),
            },
            None,
        )
        .unwrap();
    state.notifications.observe(route, Lifecycle::EndSession);
    state.stopped = true;
    files.write(&state).unwrap();
    let mut hook = Hook::read(
        json!({"session_id":"session","cwd":"/work","hook_event_name":"UserPromptSubmit","prompt":marker}),
    )
    .unwrap();
    super::resume(&files, &hook).unwrap();
    assert!(files.read().unwrap().stopped);
    hook.0["prompt"] = json!("Continue");
    super::resume(&files, &hook).unwrap();
    assert!(!files.read().unwrap().stopped);
    std::fs::remove_dir_all(root).unwrap();
}

/// A child that runs until it is stopped, standing in for a flush that hangs.
#[cfg(unix)]
fn hanging() -> super::Result<std::process::Child> {
    std::process::Command::new("sleep")
        .arg("600")
        .spawn()
        .map_err(|e| e.to_string())
}

#[cfg(unix)]
#[test]
fn the_worker_starts_no_flush_while_one_it_started_runs_and_stops_one_after_6_minutes() {
    let clock = SystemClock::with_epoch(Some("0")).passing_if(true);
    let mut flushing = super::flushing::Flushing::default();
    let started = std::cell::Cell::new(0);
    let start = || {
        started.set(started.get() + 1);
        hanging()
    };
    flushing.tick(&clock, true, || true, start);
    assert!(flushing.runs());
    clock.wait(std::time::Duration::from_secs(5));
    flushing.tick(&clock, true, || true, start);
    assert_eq!(started.get(), 1);
    clock.wait(super::flushing::MOST_TIME);
    flushing.tick(&clock, false, || true, start);
    assert!(!flushing.runs());
    flushing.tick(&clock, true, || true, start);
    assert_eq!(started.get(), 2);
}

#[cfg(unix)]
#[test]
fn the_worker_starts_a_flush_only_on_a_beat_where_one_is_due() {
    let clock = SystemClock::with_epoch(Some("0")).passing_if(true);
    let mut flushing = super::flushing::Flushing::default();
    flushing.tick(&clock, true, || false, hanging);
    assert!(!flushing.runs());
    flushing.tick(&clock, false, || true, hanging);
    assert!(!flushing.runs());
    flushing.tick(&clock, true, || true, hanging);
    assert!(flushing.runs());
}

#[cfg(unix)]
#[test]
fn a_flush_the_worker_started_that_has_ended_is_forgotten() {
    let clock = SystemClock::with_epoch(Some("0")).passing_if(true);
    let mut flushing = super::flushing::Flushing::default();
    let quick = || {
        std::process::Command::new("true")
            .spawn()
            .map_err(|e| e.to_string())
    };
    flushing.tick(&clock, true, || true, quick);
    assert!(flushing.runs());
    flushing.wait();
    flushing.tick(&clock, false, || false, quick);
    assert!(!flushing.runs());
}

#[test]
fn a_flush_the_worker_cannot_start_leaves_the_worker_running_and_the_next_due_beat_tries_again() {
    let clock = SystemClock::with_epoch(Some("0")).passing_if(true);
    let mut flushing = super::flushing::Flushing::default();
    let tries = std::cell::Cell::new(0);
    let failing = || -> super::Result<std::process::Child> {
        tries.set(tries.get() + 1);
        Err("no such file or directory".to_owned())
    };
    flushing.tick(&clock, true, || true, failing);
    assert!(!flushing.runs());
    flushing.tick(&clock, true, || true, failing);
    assert_eq!(tries.get(), 2);
}

//! Worker scheduling and acknowledgement without a network or a native host.

use loft_ports::clock::{Clock as _, SystemClock};
use serde_json::json;

use super::{drive, flush};
use crate::yard::codex::state::{Files, State};

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

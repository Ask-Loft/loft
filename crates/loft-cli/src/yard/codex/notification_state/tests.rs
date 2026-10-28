use loft_ports::clock::{Clock as _, SystemClock};

use super::*;

fn clock() -> SystemClock {
    SystemClock::with_epoch(Some("1000")).passing_if(true)
}

fn route(thread: &str) -> Route {
    Route {
        caller: Caller {
            session: "session".into(),
            agent: None,
        },
        thread: thread.into(),
        directory: "/work".into(),
    }
}

fn resolution(thread: &str) -> Resolution {
    Resolution {
        log: "log".into(),
        submission: "submission".into(),
        revision: 2,
        tree: "/work".into(),
        branch: "feat/shelf".into(),
        thread: thread.into(),
    }
}

#[test]
fn an_unread_resolution_can_be_claimed_only_by_its_armed_caller() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    assert!(ledger.offer(resolution("unknown"), None).is_none());
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    assert!(ledger.claim(&marker, clock.now().unwrap()).is_some());
}

#[test]
fn retry_backoff_advances_with_the_passing_clock_and_survives_serialization() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    let first = ledger.claim(&marker, clock.now().unwrap()).unwrap();
    assert!(!first.needs_receipt);
    assert_eq!(first.marker, marker);
    assert_eq!(first.resolution.revision, 2);
    let saved = serde_json::to_vec(&ledger).unwrap();
    let mut restarted: Ledger = serde_json::from_slice(&saved).unwrap();
    clock.wait(std::time::Duration::from_secs(4));
    assert!(restarted.claim(&marker, clock.now().unwrap()).is_none());
    clock.wait(std::time::Duration::from_secs(1));
    let retry = restarted.claim(&marker, clock.now().unwrap()).unwrap();
    assert!(retry.needs_receipt);
    assert_eq!(retry.marker, first.marker);
}

#[test]
fn acknowledged_or_explicitly_read_resolutions_are_not_offered_after_restart() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    let attempt = ledger.claim(&marker, clock.now().unwrap()).unwrap();
    ledger.confirm(&attempt);
    let mut restarted: Ledger =
        serde_json::from_slice(&serde_json::to_vec(&ledger).unwrap()).unwrap();
    assert!(restarted.offer(resolution("thread"), None).is_none());
    clock.wait(std::time::Duration::from_secs(60));
    assert!(restarted.claim(&marker, clock.now().unwrap()).is_none());
    assert!(restarted.offer(resolution("thread"), Some(2)).is_none());
    assert_eq!(restarted.deliveries[&marker].progress, Progress::Read);
}

#[test]
fn an_explicit_refusal_retries_after_backoff_without_claiming_a_native_receipt() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    let first = ledger.claim(&marker, clock.now().unwrap()).unwrap();
    ledger.refuse(&first);
    assert!(ledger.claim(&marker, clock.now().unwrap()).is_none());
    clock.wait(std::time::Duration::from_secs(5));
    assert!(
        !ledger
            .claim(&marker, clock.now().unwrap())
            .unwrap()
            .needs_receipt
    );
}

#[test]
fn interruption_cancels_pending_work_and_a_late_result_cannot_rearm_it() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    let attempt = ledger.claim(&marker, clock.now().unwrap()).unwrap();
    ledger.observe(route("thread"), Lifecycle::Disarm);
    ledger.confirm(&attempt);
    ledger.refuse(&attempt);
    assert!(!ledger.current(&attempt));
    assert_eq!(ledger.deliveries[&marker].progress, Progress::Cancelled);
    let mut restarted: Ledger =
        serde_json::from_slice(&serde_json::to_vec(&ledger).unwrap()).unwrap();
    restarted.observe(route("thread"), Lifecycle::Start);
    restarted.observe(route("thread"), Lifecycle::Other);
    assert!(restarted.offer(resolution("thread"), None).is_none());
    clock.wait(std::time::Duration::from_secs(60));
    assert!(restarted.claim(&marker, clock.now().unwrap()).is_none());
    restarted.observe(route("thread"), Lifecycle::UserPrompt);
    assert_eq!(
        restarted.offer(resolution("thread"), None),
        Some(marker.clone())
    );
    assert!(restarted.claim(&marker, clock.now().unwrap()).is_some());
}

#[test]
fn normal_turn_end_keeps_monitoring_but_agent_end_does_not_disarm_another_caller() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("main"), Lifecycle::Start);
    let mut sub = route("sub");
    sub.caller.agent = Some(("sub".into(), "review".into()));
    ledger.observe(sub.clone(), Lifecycle::Start);
    ledger.observe(route("main"), Lifecycle::Other);
    ledger.observe(sub, Lifecycle::Disarm);
    assert!(ledger.offer(resolution("sub"), None).is_none());
    let marker = ledger.offer(resolution("main"), None).unwrap();
    assert!(ledger.claim(&marker, clock.now().unwrap()).is_some());
    ledger.observe(route("main"), Lifecycle::EndSession);
    assert!(!ledger.callers["main"].armed);
    assert!(!ledger.callers["sub"].armed);
}

#[test]
fn closed_logs_cancel_pending_delivery_without_disabling_other_logs() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let old = ledger.offer(resolution("thread"), None).unwrap();
    let mut other = resolution("thread");
    other.log = "another-log".into();
    let current = ledger.offer(other, None).unwrap();
    ledger.cancel_log("log");
    assert!(ledger.claim(&old, clock.now().unwrap()).is_none());
    assert!(ledger.claim(&current, clock.now().unwrap()).is_some());
}

#[test]
fn availability_distinguishes_uncertain_delivery_from_a_known_available_connection() {
    let clock = clock();
    let mut ledger = Ledger::default();
    assert_eq!(ledger.availability("thread"), Availability::Unavailable);
    ledger.observe(route("thread"), Lifecycle::Start);
    assert_eq!(ledger.availability("thread"), Availability::Pending);
    ledger.report("thread", Availability::Available);
    assert_eq!(ledger.availability("thread"), Availability::Available);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    let attempt = ledger.claim(&marker, clock.now().unwrap()).unwrap();
    assert_eq!(ledger.availability("thread"), Availability::Uncertain);
    ledger.confirm(&attempt);
    assert_eq!(ledger.availability("thread"), Availability::Available);
}

#[test]
fn only_the_callers_own_read_progress_suppresses_an_unacknowledged_ticket() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    // A fallback read is not an own-read argument, even if another reader saw a later revision.
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    assert!(ledger.offer(resolution("thread"), Some(1)).is_some());
    assert!(ledger.offer(resolution("thread"), Some(2)).is_none());
    assert!(ledger.claim(&marker, clock.now().unwrap()).is_none());
    assert_eq!(ledger.deliveries[&marker].progress, Progress::Read);
}

#[test]
fn generated_prompts_are_identified_by_their_persisted_marker_and_target() {
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    let prompt = format!("{marker}\nRead the resolved submission.");
    assert!(ledger.is_notification("thread", &prompt));
    assert!(!ledger.is_notification("another-thread", &prompt));
    assert!(!ledger.is_notification("thread", "yard-decision-unknown\nA genuine user message."));
    let restarted: Ledger = serde_json::from_slice(&serde_json::to_vec(&ledger).unwrap()).unwrap();
    assert!(restarted.is_notification("thread", &prompt));
    assert!(
        !serde_json::to_string(&restarted)
            .unwrap()
            .contains("Read the resolved submission")
    );
}

#[test]
fn changing_the_native_route_invalidates_an_in_flight_result() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    let old = ledger.claim(&marker, clock.now().unwrap()).unwrap();
    let mut moved = route("thread");
    moved.directory = "/another-worktree".into();
    ledger.observe(moved, Lifecycle::UserPrompt);
    ledger.confirm(&old);
    assert!(!ledger.current(&old));
    assert_eq!(ledger.deliveries[&marker].progress, Progress::Cancelled);
}

#[test]
fn ending_one_session_does_not_disarm_an_unrelated_session() {
    let mut ledger = Ledger::default();
    ledger.observe(route("main"), Lifecycle::Start);
    let mut foreign = route("foreign");
    foreign.caller.session = "another-session".into();
    ledger.observe(foreign, Lifecycle::Start);
    ledger.observe(route("main"), Lifecycle::EndSession);
    assert!(!ledger.callers["main"].armed);
    assert!(ledger.callers["foreign"].armed);
}

#[test]
fn a_native_prompt_receipt_confirms_only_a_current_armed_delivery() {
    let clock = clock();
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    let attempt = ledger.claim(&marker, clock.now().unwrap()).unwrap();
    assert_eq!(ledger.receive_prompt("other", &marker), None);
    assert_eq!(ledger.receive_prompt("thread", &marker), Some(true));
    assert_eq!(ledger.deliveries[&marker].progress, Progress::Accepted);
    assert!(!ledger.current(&attempt));
    assert_eq!(ledger.availability("thread"), Availability::Available);
    ledger.observe(route("thread"), Lifecycle::Disarm);
    assert_eq!(ledger.receive_prompt("thread", &marker), Some(false));
    assert!(!ledger.callers["thread"].armed);
}

#[test]
fn a_late_native_prompt_does_not_revive_a_cancelled_or_superseded_attempt() {
    let mut ledger = Ledger::default();
    ledger.observe(route("thread"), Lifecycle::Start);
    let marker = ledger.offer(resolution("thread"), None).unwrap();
    ledger.cancel_marker(&marker);
    assert_eq!(ledger.receive_prompt("thread", &marker), Some(false));
    ledger.observe(route("thread"), Lifecycle::Disarm);
    ledger.observe(route("thread"), Lifecycle::UserPrompt);
    assert_eq!(ledger.receive_prompt("thread", &marker), Some(false));
    ledger.offer(resolution("thread"), None);
    assert_eq!(ledger.receive_prompt("thread", &marker), Some(true));
    ledger.offer(resolution("thread"), Some(2));
    assert_eq!(ledger.receive_prompt("thread", &marker), Some(true));
    assert_eq!(ledger.deliveries[&marker].progress, Progress::Read);
}

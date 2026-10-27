use super::*;
use crate::yard::codex::commands::Caller;
use crate::yard::codex::notification_state::{Lifecycle, Resolution, Route};

#[test]
fn a_late_delivery_result_does_not_report_availability_for_a_new_native_route() {
    let root = std::env::temp_dir().join(format!("codex-notify-finish-{}", uuid::Uuid::new_v4()));
    let files = Files(root.clone());
    let mut state = super::super::state::State::default();
    let mut route = Route {
        caller: Caller {
            session: "session".into(),
            agent: None,
        },
        thread: "session".into(),
        directory: "/work".into(),
    };
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
    let attempt = state.notifications.claim(&marker, 1000).unwrap();
    route.directory = "/other".into();
    state.notifications.observe(route, Lifecycle::UserPrompt);
    files.write(&state).unwrap();
    finish(&files, &attempt, Ok(())).unwrap();
    let state = files.read().unwrap();
    assert_eq!(
        state.notifications.availability("session"),
        Availability::Pending
    );
    assert_eq!(
        state.notifications.deliveries[&marker].progress,
        Progress::Cancelled
    );
    std::fs::remove_dir_all(root).unwrap();
}

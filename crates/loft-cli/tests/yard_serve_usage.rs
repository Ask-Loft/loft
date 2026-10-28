//! What the page server makes of usage it cannot count: a counter it cannot read is unreported,
//! and the event that carried it is kept all the same.

use serde_json::json;

use crate::yard_serve_events::{event, open, post, session_file};

#[test]
fn a_request_with_an_unusable_counter_is_recorded_and_that_counter_is_unreported() {
    let (yard, served) = open();
    post(
        &served,
        &[
            event(
                &yard,
                "request",
                5,
                &json!({ "model": "opus", "tokens": { "input": 10, "cacheRead": 0, "cacheWrite": 0, "output": 2 } }),
            ),
            event(
                &yard,
                "request",
                6,
                &json!({ "model": "opus", "tokens": { "input": null, "cacheRead": 1.5, "cacheWrite": "3", "output": 4 } }),
            ),
        ],
    );
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert!(
        text.contains(
            "tokens:\n  input: 10\n  cacheRead: 0\n  cacheWrite: 0\n  output: 6\n  partial:\n    - input\n    - cacheRead\n    - cacheWrite\n"
        ),
        "{text}"
    );
    let events = yard.scratch.home().join(".yard/events/s1.jsonl");
    assert_eq!(std::fs::read_to_string(events).unwrap().lines().count(), 2);
}

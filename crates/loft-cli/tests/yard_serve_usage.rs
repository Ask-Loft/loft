//! What the page server makes of usage it cannot count: a counter it cannot read is unreported,
//! and the event that carried it is kept all the same; a request that reports no tokens leaves
//! the totals partial, and a compaction that reports none adds nothing.

use serde_json::json;

use crate::yard_serve_events::{event, open, post, session_file, tokens};

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

#[test]
fn a_compaction_that_reports_no_tokens_adds_nothing_and_leaves_the_totals_complete() {
    let (yard, served) = open();
    post(
        &served,
        &[
            event(
                &yard,
                "request",
                5,
                &json!({ "model": "opus", "tokens": tokens(10, 1, 2, 3) }),
            ),
            event(
                &yard,
                "compaction",
                9,
                &json!({ "before": 900, "after": 100 }),
            ),
        ],
    );
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert!(text.contains("lastEnded: 2026-01-01T10:00:05Z\n"), "{text}");
    assert!(
        text.contains(
            "tokens:\n  input: 10\n  cacheRead: 1\n  cacheWrite: 2\n  output: 3\nstages:"
        ),
        "{text}"
    );
    assert!(!text.contains("partial"), "{text}");
    assert!(!text.contains("model: unknown"), "{text}");
}

#[test]
fn a_request_that_reports_no_tokens_makes_the_totals_partial() {
    let (yard, served) = open();
    post(
        &served,
        &[
            event(
                &yard,
                "request",
                5,
                &json!({ "model": "opus", "tokens": tokens(10, 1, 2, 3) }),
            ),
            event(&yard, "request", 9, &json!({ "model": "opus" })),
        ],
    );
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert!(
        text.contains(
            "tokens:\n  input: 10\n  cacheRead: 1\n  cacheWrite: 2\n  output: 3\n  partial:\n    - input\n    - cacheRead\n    - cacheWrite\n    - output\n"
        ),
        "{text}"
    );
}

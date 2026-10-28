//! The Codex adapter's queue, delivered to the actual local page server: a request queued by an
//! adapter that did not classify cache writes is classified by the adapter's rule as it is
//! posted, and one the page server already holds is not counted twice.

use std::fs;
use std::time::Duration;

use serde_json::{Value, json};

use crate::served::{Served, within};
use crate::yard::{ASK, BRANCH, Yard};
use crate::yard_codex::{hook, state, watch};
use crate::yard_serve_events::{event, post};

/// A request queued by an adapter that did not classify cache writes: a subscription account's
/// zero count in its tokens.
fn unclassified(yard: &Yard) -> Value {
    event(
        yard,
        "request",
        1,
        &json!({
            "host": "codex",
            "eventId": "queued",
            "model": "fixture",
            "tokens": {"input": 70, "cacheRead": 30, "cacheWrite": 0, "output": 5},
            "usageSource": {"account": "chatgpt"},
        }),
    )
}

/// Yard Log with session `s1`'s stage active, a page server, and the Codex session's state.
fn start() -> (Yard, Served) {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--host", "codex", "--send"]);
    let served = Served::start(&yard, &["--tree", yard.scratch.0.to_str().unwrap()]);
    hook(&yard, &crate::yard_codex::event(&yard, "UserPromptSubmit"));
    (yard, served)
}

/// Puts `request` in the adapter's queue `field`: `events`, or the detached `in_flight` batch.
fn queue(yard: &Yard, field: &str, request: &Value) {
    let mut kept = state(yard);
    kept[field].as_array_mut().unwrap().push(request.clone());
    let path = yard
        .scratch
        .home()
        .join(".yard/codex/codex-session/state.json");
    fs::write(path, kept.to_string()).unwrap();
}

/// Delivers the adapter's queue to `served` and waits until the queue is empty.
fn deliver(yard: &Yard, served: &Served) {
    let input = crate::yard_codex::event(yard, "UserPromptSubmit");
    let _worker = watch(
        yard,
        &input,
        &yard.clock,
        &["--port", &served.port.to_string()],
    );
    let emptied = within(Duration::from_secs(5), || {
        let kept = state(yard);
        kept["events"] == json!([]) && kept["in_flight"] == json!([])
    });
    assert!(emptied, "{}", state(yard));
}

/// Session `s1`'s requests, as the page server keeps them.
fn requests(yard: &Yard) -> Vec<Value> {
    let path = yard.scratch.home().join(".yard/events/codex~s1.jsonl");
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|line| line["kind"] == "request")
        .collect()
}

/// The tokens of every session, as the page server sums them.
fn summed(yard: &Yard, served: &Served) -> Value {
    let tree = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let reply = served.get(&format!("/api/log?tree={tree}&branch={BRANCH}"));
    reply.json()["tokenSums"]["all"].clone()
}

#[test]
fn a_request_queued_before_the_upgrade_reaches_the_page_server_classified() {
    let (yard, served) = start();
    queue(&yard, "events", &unclassified(&yard));
    deliver(&yard, &served);
    let kept = requests(&yard);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(kept[0]["tokens"].get("cacheWrite").is_none(), "{}", kept[0]);
    assert_eq!(
        kept[0]["usageSource"],
        json!({"account":"chatgpt","reported":{"cacheWrite":0}})
    );
    let sums = summed(&yard, &served);
    assert!(sums.get("cacheWrite").is_none(), "{sums}");
    assert_eq!(sums["input"], 70);
}

#[test]
fn a_queued_request_the_page_server_already_holds_leaves_the_queue_and_is_counted_once() {
    let (yard, served) = start();
    let held = unclassified(&yard);
    assert_eq!(post(&served, std::slice::from_ref(&held))["recorded"], 1);
    queue(&yard, "in_flight", &held);
    deliver(&yard, &served);
    let kept = requests(&yard);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0]["eventId"], "queued");
    assert_eq!(summed(&yard, &served)["input"], 70);
}

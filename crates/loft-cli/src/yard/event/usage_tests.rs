use super::Event;
use serde_json::{Value, json};

fn source(host: &str, account: &str, write: &Value) -> Value {
    json!({"host":host,"session":"session","dir":"/work","at":"2026-01-01T00:00:00Z","kind":"request",
        "tokens":{"input":70,"cacheRead":30,"cacheWrite":write,"output":5},"usageSource":{"account":account}})
}

#[test]
fn a_zero_cache_write_is_counted_as_given_whatever_the_host_or_account() {
    for host in ["codex", "claude-code"] {
        for account in [
            Some("chatgpt"),
            Some("unknown"),
            Some("future"),
            Some("apiKey"),
            None,
        ] {
            let mut posted = source(host, account.unwrap_or_default(), &json!(0));
            if account.is_none() {
                posted.as_object_mut().unwrap().remove("usageSource");
            }
            let read = Event::read(&posted).unwrap();
            let tokens = read.tokens().unwrap().json();
            assert_eq!(tokens["cacheWrite"], 0, "{host} {account:?}: {tokens}");
            assert_eq!(read.posted, posted);
        }
    }
}

#[test]
fn positive_codex_counts_and_claude_measured_zeros_remain_measured() {
    for (host, account, write) in [
        ("codex", "chatgpt", 7),
        ("codex", "unknown", 7),
        ("codex", "apiKey", 7),
        ("claude-code", "unknown", 0),
    ] {
        let posted = source(host, account, &json!(write));
        let read = Event::read(&posted).unwrap();
        assert_eq!(read.tokens().unwrap().fields().len(), 4);
        assert_eq!(read.posted, posted);
    }
}

#[test]
fn missing_or_malformed_source_counters_are_not_replaced_with_measured_zeros() {
    let mut missing = source("codex", "apiKey", &Value::Null);
    missing["tokens"]
        .as_object_mut()
        .unwrap()
        .remove("cacheWrite");
    assert_eq!(
        Event::read(&missing)
            .unwrap()
            .tokens()
            .unwrap()
            .fields()
            .len(),
        3
    );
}

#[test]
fn an_unusable_counter_is_unreported_and_its_event_is_still_read() {
    for unusable in [Value::Null, json!(-1), json!(2.5), json!("0")] {
        let posted = source("codex", "chatgpt", &unusable);
        let read = Event::read(&posted).unwrap();
        let fields = read.tokens().unwrap().fields();
        let names: Vec<_> = fields.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["input", "cacheRead", "output"], "{unusable}");
        assert_eq!(read.posted, posted);
    }
    for unusable in [Value::Null, json!("many"), json!([1, 2])] {
        let mut posted = source("claude-code", "unknown", &json!(0));
        posted["tokens"] = unusable;
        let read = Event::read(&posted).unwrap();
        assert_eq!(read.tokens(), Some(super::Tokens::unreported()));
    }
}

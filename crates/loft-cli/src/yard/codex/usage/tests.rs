use serde_json::{Value, json};

use super::{native, observe};
use crate::yard::codex::{state::State, translate::Hook};

fn state() -> State {
    let mut state = State::default();
    native(&Hook::read(json!({"hook_event_name":"UserPromptSubmit","session_id":"thread","turn_id":"turn","cwd":"/tree","model":"model"})).unwrap(), &mut state);
    state
}
fn usage(input: u64, cached: u64, output: u64) -> Value {
    let counters = json!({"inputTokens":input,"cachedInputTokens":cached,"outputTokens":output,"reasoningOutputTokens":2,"totalTokens":input+output});
    json!({"last":counters,"total":counters})
}

#[test]
fn latest_request_usage_separates_fresh_cached_and_output_without_recounting_reasoning() {
    let mut state = state();
    let mut usage = usage(100, 30, 5);
    usage["last"]["cacheWriteInputTokens"] = json!(7);
    assert!(observe(&mut state, "thread", "turn", &usage, "now"));
    assert_eq!(
        state.events[0]["tokens"],
        json!({"input":70,"cacheRead":30,"cacheWrite":7,"output":5})
    );
    assert_eq!(state.events[0]["model"], "model");
}

#[test]
fn absent_cache_write_stays_unavailable_and_cumulative_history_is_not_imported() {
    let mut state = state();
    let mut observation = usage(100, 30, 5);
    observation["total"] = usage(10000, 3000, 500)["total"].clone();
    assert!(observe(&mut state, "thread", "turn", &observation, "now"));
    assert_eq!(
        state.events[0]["tokens"],
        json!({"input":70,"cacheRead":30,"output":5})
    );
    assert_eq!(state.events.len(), 1);
}

#[test]
fn repeated_snapshots_resets_and_context_fill_preserve_exact_request_charges() {
    let mut state = state();
    let first = usage(100, 30, 5);
    observe(&mut state, "thread", "turn", &first, "one");
    let id = state.events[0]["eventId"].clone();
    let persisted = serde_json::to_vec(&state).unwrap();
    let mut state: State = serde_json::from_slice(&persisted).unwrap();
    observe(&mut state, "thread", "turn", &first, "retry");
    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0]["at"], "one");
    let mut fill = usage(0, 0, 0);
    fill["total"]["totalTokens"] = json!(258_400);
    observe(&mut state, "thread", "turn", &fill, "fill");
    assert_eq!(state.events.len(), 1);
    observe(&mut state, "thread", "turn", &first, "after-reset");
    assert_eq!(state.events.len(), 2);
    assert_ne!(state.events[1]["eventId"], id);
    assert_eq!(state.usage.cursors["thread"].generation, 1);
}

#[test]
fn invalid_or_foreign_usage_advances_neither_cursor_nor_outbox() {
    let mut state = state();
    for (field, value) in [
        ("inputTokens", json!(-1)),
        ("cachedInputTokens", json!(101)),
        ("outputTokens", json!(-1)),
        ("reasoningOutputTokens", json!(-1)),
        ("cacheWriteInputTokens", json!(-1)),
    ] {
        let mut invalid = usage(100, 30, 5);
        invalid["last"][field] = value;
        assert!(!observe(&mut state, "thread", "turn", &invalid, "now"));
    }
    assert!(!observe(
        &mut state,
        "foreign",
        "turn",
        &usage(100, 30, 5),
        "now"
    ));
    assert!(!observe(
        &mut state,
        "thread",
        "foreign",
        &usage(100, 30, 5),
        "now"
    ));
    assert_eq!(state.events, Vec::<Value>::new());
    assert!(state.usage.cursors.is_empty());
}

#[test]
fn skipped_observations_record_one_unavailable_request_without_guessing_counts() {
    let mut state = state();
    observe(&mut state, "thread", "turn", &usage(100, 30, 5), "one");
    let mut next = usage(100, 30, 5);
    next["total"] = usage(300, 90, 15)["total"].clone();
    observe(&mut state, "thread", "turn", &next, "three");
    assert_eq!(state.events.len(), 3);
    assert!(state.events[1].get("tokens").is_none());
    assert_eq!(state.events[2]["tokens"]["input"], 70);
    super::gap(&mut state, "thread", "again");
    assert_eq!(state.events.len(), 3);
}

/// The state after one request is observed for `account`, where one is given, with `write`
/// cache writes in its latest and cumulative counts, where given.
fn observed(account: Option<&str>, write: Option<u64>) -> State {
    let mut state = state();
    let mut counters = usage(100, 30, 5);
    if let Some(account) = account {
        counters["account"] = json!(account);
    }
    if let Some(write) = write {
        counters["last"]["cacheWriteInputTokens"] = json!(write);
        counters["total"]["cacheWriteInputTokens"] = json!(write);
    }
    assert!(observe(&mut state, "thread", "turn", &counters, "now"));
    if let Some(write) = write {
        let kept = serde_json::to_value(&state.usage.cursors["thread"]).unwrap();
        assert_eq!(kept["snapshot"]["write"], write);
    }
    state
}

#[test]
fn a_zero_cache_write_from_a_verified_api_key_account_is_measured_in_the_tokens() {
    let state = observed(Some("apiKey"), Some(0));
    assert_eq!(state.events[0]["tokens"]["cacheWrite"], 0);
    assert_eq!(state.events[0]["usageSource"], json!({"account":"apiKey"}));
}

#[test]
fn an_unverified_zero_cache_write_leaves_the_tokens_and_is_reported_under_its_source() {
    for (account, kept) in [
        (Some("chatgpt"), "chatgpt"),
        (Some("unknown"), "unknown"),
        (None, "unknown"),
    ] {
        let state = observed(account, Some(0));
        let tokens = &state.events[0]["tokens"];
        assert!(tokens.get("cacheWrite").is_none(), "{tokens}");
        assert_eq!(tokens["input"], 70);
        assert_eq!(
            state.events[0]["usageSource"],
            json!({"account":kept,"reported":{"cacheWrite":0}})
        );
    }
}

#[test]
fn a_positive_cache_write_is_measured_for_every_account() {
    for account in ["apiKey", "chatgpt", "unknown"] {
        let state = observed(Some(account), Some(4));
        assert_eq!(state.events[0]["tokens"]["cacheWrite"], 4);
        assert_eq!(state.events[0]["usageSource"], json!({"account":account}));
    }
}

#[test]
fn an_absent_cache_write_is_neither_measured_nor_reported_even_for_an_api_key_account() {
    let state = observed(Some("apiKey"), None);
    assert!(state.events[0]["tokens"].get("cacheWrite").is_none());
    assert_eq!(state.events[0]["usageSource"], json!({"account":"apiKey"}));
}

fn queued(account: Option<&str>, write: u64) -> Value {
    let mut event = json!({"kind":"request","eventId":"id","tokens":{"input":70,"cacheRead":30,"cacheWrite":write,"output":5}});
    if let Some(account) = account {
        event["usageSource"] = json!({"account":account});
    }
    event
}

#[test]
fn the_rule_keeps_a_zero_cache_write_in_an_events_tokens_only_for_a_verified_api_key_account() {
    let mut event = queued(Some("apiKey"), 0);
    assert!(!super::classify_event(&mut event));
    assert_eq!(event, queued(Some("apiKey"), 0));
}

#[test]
fn the_rule_moves_an_unverified_zero_cache_write_of_an_event_under_its_source() {
    for (account, kept) in [
        (
            Some("chatgpt"),
            json!({"account":"chatgpt","reported":{"cacheWrite":0}}),
        ),
        (
            Some("unknown"),
            json!({"account":"unknown","reported":{"cacheWrite":0}}),
        ),
        (None, json!({"reported":{"cacheWrite":0}})),
    ] {
        let mut event = queued(account, 0);
        assert!(super::classify_event(&mut event));
        assert_eq!(
            event["tokens"],
            json!({"input":70,"cacheRead":30,"output":5})
        );
        assert_eq!(event["usageSource"], kept);
        assert_eq!(event["eventId"], "id");
        assert!(
            !super::classify_event(&mut event),
            "a second pass moves nothing"
        );
    }
}

#[test]
fn the_rule_keeps_a_positive_cache_write_of_an_event_and_leaves_other_kinds_alone() {
    for account in [Some("apiKey"), Some("chatgpt"), None] {
        let mut event = queued(account, 4);
        assert!(!super::classify_event(&mut event));
        assert_eq!(event, queued(account, 4));
    }
    let mut prompt = queued(None, 0);
    prompt["kind"] = json!("prompt");
    assert!(!super::classify_event(&mut prompt));
    assert!(prompt.get("usageSource").is_none());
    let mut compaction = queued(Some("chatgpt"), 0);
    compaction["kind"] = json!("compaction");
    assert!(super::classify_event(&mut compaction));
}

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

fn zero_write(account: Option<&str>) -> State {
    let mut state = state();
    let mut counters = usage(100, 30, 5);
    if let Some(account) = account {
        counters["account"] = json!(account);
    }
    counters["last"]["cacheWriteInputTokens"] = json!(0);
    counters["total"]["cacheWriteInputTokens"] = json!(0);
    assert!(observe(&mut state, "thread", "turn", &counters, "now"));
    let kept = serde_json::to_value(&state.usage.cursors["thread"]).unwrap();
    assert_eq!(kept["snapshot"]["write"], 0);
    state
}

#[test]
fn a_zero_cache_write_from_a_verified_api_key_account_is_measured_in_the_tokens() {
    let state = zero_write(Some("apiKey"));
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
        let state = zero_write(account);
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
        let mut state = state();
        let mut counters = usage(100, 30, 5);
        counters["account"] = json!(account);
        counters["last"]["cacheWriteInputTokens"] = json!(4);
        assert!(observe(&mut state, "thread", "turn", &counters, "now"));
        assert_eq!(state.events[0]["tokens"]["cacheWrite"], 4);
        assert_eq!(state.events[0]["usageSource"], json!({"account":account}));
    }
}

#[test]
fn an_absent_cache_write_is_neither_measured_nor_reported_even_for_an_api_key_account() {
    let mut state = state();
    let mut counters = usage(100, 30, 5);
    counters["account"] = json!("apiKey");
    assert!(observe(&mut state, "thread", "turn", &counters, "now"));
    assert!(state.events[0]["tokens"].get("cacheWrite").is_none());
    assert_eq!(state.events[0]["usageSource"], json!({"account":"apiKey"}));
}

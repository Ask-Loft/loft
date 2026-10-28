//! Persisted accounting metadata. Raw app-server messages never enter the outbox.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::account::Account;
use super::state::State;
use super::translate::{Hook, HookKind, envelope};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub active: BTreeMap<String, Active>,
    pub cursors: BTreeMap<String, Cursor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Active {
    pub session: String,
    pub agent: Option<(String, String)>,
    pub turn: String,
    pub dir: String,
    pub model: String,
    pub ended: bool,
    pub source: Source,
    pub gap: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    pub generation: u64,
    pub snapshot: Counters,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    input: u64,
    cached: u64,
    write: Option<u64>,
    output: u64,
}

impl Counters {
    fn read(value: &Value) -> Option<Self> {
        let read = Self {
            input: value["inputTokens"].as_u64()?,
            cached: value["cachedInputTokens"].as_u64()?,
            write: match value.get("cacheWriteInputTokens") {
                Some(value) => Some(value.as_u64()?),
                None => None,
            },
            output: value["outputTokens"].as_u64()?,
        };
        if read.cached > read.input {
            return None;
        }
        for key in ["reasoningOutputTokens", "totalTokens"] {
            if value.get(key).is_some_and(|v| v.as_u64().is_none()) {
                return None;
            }
        }
        Some(read)
    }
    fn zero(&self) -> bool {
        self.input == 0 && self.output == 0 && self.write.unwrap_or(0) == 0
    }
    fn reset(&self, previous: &Self) -> bool {
        self.input < previous.input
            || self.cached < previous.cached
            || self.output < previous.output
            || self.write.zip(previous.write).is_some_and(|(a, b)| a < b)
    }
    fn skips(&self, previous: &Self, last: &Self) -> bool {
        self.input.saturating_sub(previous.input) > last.input
            || self.output.saturating_sub(previous.output) > last.output
    }
    /// The tokens and their source, classified by [`classify`].
    fn tokens(&self, account: Account) -> (Value, Value) {
        let mut tokens =
            json!({"input":self.input-self.cached,"cacheRead":self.cached,"output":self.output});
        let mut source = json!({"account":account});
        if let Some(write) = self.write {
            tokens["cacheWrite"] = json!(write);
        }
        classify(&mut tokens, &mut source, account);
        (tokens, source)
    }
}

/// The rule of a measured cache-write count, and the only one `loft` keeps: a count is measured
/// when positive, or when a verified API-key `account` reports zero. Any other zero leaves
/// `tokens` for `source`'s `reported`; gives whether it moved one.
pub fn classify(tokens: &mut Value, source: &mut Value, account: Account) -> bool {
    if account == Account::ApiKey || tokens["cacheWrite"] != 0 {
        return false;
    }
    let Some(write) = tokens.as_object_mut().and_then(|t| t.remove("cacheWrite")) else {
        return false;
    };
    if !source.is_object() {
        *source = json!({});
    }
    source["reported"] = json!({ "cacheWrite": write });
    true
}

/// [`classify`] applied to a request or compaction `event`, by the account its own
/// `usageSource` names; every other field is kept. Gives whether it moved a count.
pub fn classify_event(event: &mut Value) -> bool {
    if !matches!(event["kind"].as_str(), Some("request" | "compaction")) {
        return false;
    }
    let account = Account::source(&event["usageSource"]["account"]);
    let Some(fields) = event.as_object_mut() else {
        return false;
    };
    let (Some(mut tokens), mut source) = (
        fields.get("tokens").cloned(),
        fields.get("usageSource").cloned().unwrap_or(Value::Null),
    ) else {
        return false;
    };
    if !classify(&mut tokens, &mut source, account) {
        return false;
    }
    fields.insert("tokens".to_owned(), tokens);
    fields.insert("usageSource".to_owned(), source);
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Pending,
    Available,
    Unavailable,
}

pub fn native(hook: &Hook, state: &mut State) {
    let thread = hook.thread();
    if hook.kind() == HookKind::SessionEnd {
        for turn in state.usage.active.values_mut() {
            turn.ended = true;
        }
        return;
    }
    if matches!(
        hook.kind(),
        HookKind::Stop | HookKind::Interrupt | HookKind::SubagentStop
    ) {
        if let Some(turn) = state.usage.active.get_mut(thread)
            && hook.0["turn_id"].as_str().is_none_or(|id| id == turn.turn)
        {
            turn.ended = true;
        }
        return;
    }
    if hook.kind() != HookKind::UserPromptSubmit {
        return;
    }
    let Some(caller) = hook.caller(state) else {
        return;
    };
    let Some(turn) = hook.0["turn_id"].as_str().filter(|id| !id.is_empty()) else {
        return;
    };
    if state
        .usage
        .active
        .get(thread)
        .is_some_and(|active| active.turn == turn)
    {
        return;
    }
    state.usage.active.insert(
        thread.into(),
        Active {
            session: caller.session,
            agent: caller.agent,
            turn: turn.into(),
            dir: hook.dir().into(),
            model: hook.0["model"].as_str().unwrap_or("unknown").into(),
            ended: false,
            source: Source::Pending,
            gap: false,
        },
    );
}

fn event(turn: &Active, at: &str, id: &str) -> Value {
    let mut event = envelope(
        "request",
        (&turn.session, &turn.dir),
        turn.agent.as_ref(),
        Some(&turn.turn),
        at,
    );
    event["eventId"] = json!(id);
    event["model"] = json!(turn.model);
    event
}

fn identity(value: &Value) -> String {
    format!("codex-usage-{}", crate::yard::digest::json_hex(value))
}

pub fn gap(state: &mut State, thread: &str, at: &str) {
    let Some(turn) = state.usage.active.get_mut(thread) else {
        return;
    };
    if turn.gap {
        return;
    }
    turn.gap = true;
    let id = identity(&json!([thread, turn.turn, "gap"]));
    let event = event(turn, at, &id);
    state.keep(vec![event]);
}

/// Queue and checkpoint in memory together; the caller persists both under `Files::lock`.
pub fn observe(state: &mut State, thread: &str, turn: &str, usage: &Value, at: &str) -> bool {
    let Some(active) = state
        .usage
        .active
        .get(thread)
        .filter(|active| active.turn == turn)
    else {
        return false;
    };
    let (Some(last), Some(total)) = (
        Counters::read(&usage["last"]),
        Counters::read(&usage["total"]),
    ) else {
        return false;
    };
    let previous = state.usage.cursors.get(thread);
    if previous.is_some_and(|p| p.snapshot == total) {
        return true;
    }
    let reset = previous.is_some_and(|p| total.reset(&p.snapshot));
    let generation = previous.map_or(0, |p| p.generation + u64::from(reset));
    let skipped = !reset && previous.is_some_and(|p| total.skips(&p.snapshot, &last));
    let id = identity(&json!([thread, turn, generation, total]));
    let mut request = event(active, at, &id);
    (request["tokens"], request["usageSource"]) = last.tokens(Account::source(&usage["account"]));
    if skipped {
        gap(state, thread, at);
    }
    state.usage.cursors.insert(
        thread.into(),
        Cursor {
            generation,
            snapshot: total,
        },
    );
    if !last.zero() {
        state.keep(vec![request]);
    }
    true
}

#[cfg(test)]
mod tests;

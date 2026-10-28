//! Whitelisted native fields; raw hook input never reaches an event or adapter state.

use std::path::Path;

use serde_json::{Value, json};
use yardlog_store::session::is_session_id;

use super::Result;
use super::commands::Caller;
use super::state::State;

pub struct Hook(pub Value);

impl Hook {
    pub fn read(value: Value) -> Result<Self> {
        let valid = value["session_id"].as_str().is_some_and(is_session_id)
            && value["cwd"]
                .as_str()
                .is_some_and(|path| Path::new(path).is_absolute())
            && value["hook_event_name"]
                .as_str()
                .is_some_and(|name| !name.is_empty());
        if !valid {
            return Err(
                "a hook needs a valid session_id, absolute cwd and hook_event_name".to_owned(),
            );
        }
        if let Some(agent) = value.get("agent_id")
            && !agent.as_str().is_some_and(is_session_id)
        {
            return Err("a hook names an invalid agent_id".to_owned());
        }
        Ok(Self(value))
    }

    pub fn session(&self) -> &str {
        self.0["session_id"].as_str().unwrap_or_default()
    }
    pub fn dir(&self) -> &str {
        self.0["cwd"].as_str().unwrap_or_default()
    }
    pub fn name(&self) -> &str {
        self.0["hook_event_name"].as_str().unwrap_or_default()
    }

    pub fn supported(&self) -> bool {
        matches!(
            self.name(),
            "SessionStart"
                | "SessionEnd"
                | "UserPromptSubmit"
                | "PreToolUse"
                | "PostToolUse"
                | "SubagentStart"
                | "SubagentStop"
                | "PostCompact"
                | "Stop"
                | "Interrupt"
        )
    }

    pub fn caller(&self, state: &State) -> Option<Caller> {
        let agent = self.0["agent_id"].as_str();
        // Native tool hooks do not consistently identify a subagent. During concurrent
        // subagent work an unqualified tool event cannot safely name the main agent.
        if agent.is_none()
            && !state.subagents.is_empty()
            && matches!(
                self.name(),
                "PreToolUse" | "PostToolUse" | "PostCompact" | "Stop" | "Interrupt"
            )
        {
            return None;
        }
        if agent.is_none() && matches!(self.name(), "SubagentStart" | "SubagentStop") {
            return None;
        }
        let kind = agent.and_then(|id| {
            self.0["agent_type"]
                .as_str()
                .or_else(|| state.subagents.get(id).map(String::as_str))
        });
        if agent.is_some() && kind.is_none() {
            return None;
        }
        Some(Caller {
            session: self.session().to_owned(),
            agent: agent
                .zip(kind)
                .map(|(id, kind)| (id.to_owned(), kind.to_owned())),
        })
    }
}

/// Supported counters remain distinct from whether a usage source is currently available.
pub fn descriptor() -> Value {
    json!({"format":"yard-adapter/1", "version":crate::version::VERSION, "capabilities":{
        "events":["turn.start","turn.end","request","tool.start","tool.end","subagent.start","subagent.end","compaction","plan.start","plan.end"],
        "nativeQuestions":false,"nativePlans":false,"taskMirroring":true,"usage":["input","cacheRead","cacheWrite","output"]
    }})
}

pub fn events(hook: &Hook, state: &mut State) -> Result<Vec<Value>> {
    if hook.name() == "SubagentStart"
        && let (Some(id), Some(kind)) = (hook.0["agent_id"].as_str(), hook.0["agent_type"].as_str())
    {
        state.subagents.insert(id.to_owned(), kind.to_owned());
    }
    if matches!(hook.name(), "SessionStart" | "UserPromptSubmit") {
        state.stopped = false;
    }
    if hook.name() == "SessionEnd" {
        state.stopped = true;
        state.subagents.clear();
    }
    let Some(caller) = hook.caller(state) else {
        return Ok(vec![]);
    };
    let at = crate::yard::now().map_err(super::message)?.to_string();
    let base = |kind: &str| {
        let mut event = json!({"kind":kind,"host":"codex","session":hook.session(),"dir":hook.dir(),"at":at,"adapter":descriptor()});
        if let Some((id, kind)) = &caller.agent {
            event["agent"] = json!({"id":id,"type":kind});
        }
        if let Some(turn) = hook.0["turn_id"].as_str() {
            event["turn"] = json!(turn);
        }
        event
    };
    let mut events = vec![];
    if caller.agent.is_none()
        && let Some(mode) = hook.0["permission_mode"].as_str()
    {
        let planning = mode == "plan";
        let kind = if planning { "plan.start" } else { "plan.end" };
        if state.planning != Some(planning) && (planning || state.planning.is_some()) {
            events.push(base(kind));
        }
        state.planning = Some(planning);
    }
    let kind = match hook.name() {
        "UserPromptSubmit" => "turn.start",
        "Stop" | "Interrupt" => "turn.end",
        "SubagentStart" if caller.agent.is_some() => "subagent.start",
        "SubagentStop" if caller.agent.is_some() => "subagent.end",
        "PostCompact" => "compaction",
        "PreToolUse" => "tool.start",
        "PostToolUse" => "tool.end",
        _ => return Ok(events),
    };
    let mut event = base(kind);
    if matches!(kind, "tool.start" | "tool.end") {
        let (Some(tool), Some(call)) =
            (hook.0["tool_name"].as_str(), hook.0["tool_use_id"].as_str())
        else {
            return Ok(events);
        };
        event["tool"] = json!(tool);
        event["call"] = json!(call);
        if kind == "tool.end" {
            if let Some(code) = hook.0["tool_response"]["exit_code"].as_i64() {
                event["failed"] = json!(code != 0);
            } else if let Some(failed) = hook.0["tool_response"]["isError"].as_bool() {
                event["failed"] = json!(failed);
            }
        }
    }
    events.push(event);
    if hook.name() == "SubagentStop"
        && let Some(id) = hook.0["agent_id"].as_str()
    {
        state.subagents.remove(id);
    }
    Ok(events)
}

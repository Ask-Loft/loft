//! Whitelisted native fields; raw hook input never reaches an event or adapter state.

use std::path::Path;
use std::sync::OnceLock;

use serde_json::{Value, json};
use yardlog::identity::is_session_id;

use super::Result;
use super::commands::{Caller, HOST};
use super::state::State;

/// A native hook's input, with its event's name parsed once.
pub struct Hook(pub Value, HookKind);

/// The native hook events the adapter handles; `Other` is any it does not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HookKind {
    SessionStart,
    SubagentStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostCompact,
    Stop,
    Interrupt,
    SubagentStop,
    SessionEnd,
    Other,
}

impl HookKind {
    fn of(name: &str) -> Self {
        match name {
            "SessionStart" => Self::SessionStart,
            "SubagentStart" => Self::SubagentStart,
            "UserPromptSubmit" => Self::UserPromptSubmit,
            "PreToolUse" => Self::PreToolUse,
            "PostToolUse" => Self::PostToolUse,
            "PostCompact" => Self::PostCompact,
            "Stop" => Self::Stop,
            "Interrupt" => Self::Interrupt,
            "SubagentStop" => Self::SubagentStop,
            "SessionEnd" => Self::SessionEnd,
            _ => Self::Other,
        }
    }

    /// A session's, a subagent's or a turn's start.
    pub fn starts(self) -> bool {
        matches!(
            self,
            Self::SessionStart | Self::SubagentStart | Self::UserPromptSubmit
        )
    }

    /// A turn's, a subagent's or a session's end.
    pub fn ends(self) -> bool {
        matches!(
            self,
            Self::Stop | Self::Interrupt | Self::SubagentStop | Self::SessionEnd
        )
    }

    /// An end of the caller's work, which returns its active tasks to open: a subagent's end, an
    /// interruption or the session's end. A main agent goes on after its turn, so the main
    /// agent's `Stop` is not one.
    pub fn releases(self, subagent: bool) -> bool {
        match self {
            Self::Stop => subagent,
            kind => kind.ends(),
        }
    }
}

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
        let kind = HookKind::of(value["hook_event_name"].as_str().unwrap_or_default());
        Ok(Self(value, kind))
    }

    pub fn session(&self) -> &str {
        self.0["session_id"].as_str().unwrap_or_default()
    }
    /// The Codex thread the hook runs in: its subagent's, or the session's.
    pub fn thread(&self) -> &str {
        self.0["agent_id"]
            .as_str()
            .unwrap_or_else(|| self.session())
    }
    pub fn dir(&self) -> &str {
        self.0["cwd"].as_str().unwrap_or_default()
    }
    /// The native event's name, as Codex gave it.
    pub fn name(&self) -> &str {
        self.0["hook_event_name"].as_str().unwrap_or_default()
    }
    pub fn kind(&self) -> HookKind {
        self.1
    }

    pub fn supported(&self) -> bool {
        self.1 != HookKind::Other
    }

    pub fn caller(&self, state: &State) -> Option<Caller> {
        let agent = self.0["agent_id"].as_str();
        // Native tool hooks do not consistently identify a subagent. During concurrent
        // subagent work an unqualified tool event cannot safely name the main agent.
        if agent.is_none()
            && !state.subagents.is_empty()
            && matches!(
                self.1,
                HookKind::PreToolUse
                    | HookKind::PostToolUse
                    | HookKind::PostCompact
                    | HookKind::Stop
                    | HookKind::Interrupt
            )
        {
            return None;
        }
        if agent.is_none() && matches!(self.1, HookKind::SubagentStart | HookKind::SubagentStop) {
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

/// The adapter's descriptor, built once: every normalized event kind and every counter. Supported
/// counters remain distinct from whether a usage source is currently available.
pub fn descriptor() -> &'static Value {
    static DESCRIPTOR: OnceLock<Value> = OnceLock::new();
    DESCRIPTOR.get_or_init(|| {
        json!({"format":yardlog::adapter::FORMAT, "version":crate::version::VERSION, "capabilities":{
            "events":yardlog::adapter::EVENTS, "usage":yardlog::usage::COUNTERS,
            "nativeQuestions":false, "nativePlans":false, "taskMirroring":true
        }})
    })
}

/// An event of `kind` of the session's agent `agent` in `dir` at `at`, during `turn` where
/// there is one: the fields every event the adapter posts holds.
pub fn envelope(
    kind: &str,
    (session, dir): (&str, &str),
    agent: Option<&(String, String)>,
    turn: Option<&str>,
    at: &str,
) -> Value {
    let mut event = json!({"kind":kind,"host":HOST,"session":session,"dir":dir,"at":at});
    if let Some((id, kind)) = agent {
        event["agent"] = json!({"id":id,"type":kind});
    }
    if let Some(turn) = turn {
        event["turn"] = json!(turn);
    }
    event
}

/// Gives `event` the adapter's descriptor where it is the session's first or a turn's start.
fn describe(event: &mut Value, state: &mut State) {
    if event["kind"] == "turn.start" || !state.described {
        event["adapter"] = descriptor().clone();
        state.described = true;
    }
}

/// The events a hook posts, the session's first and each turn's start with the descriptor.
pub fn events(hook: &Hook, state: &mut State) -> Result<Vec<Value>> {
    let mut events = translated(hook, state)?;
    for event in &mut events {
        describe(event, state);
    }
    Ok(events)
}

fn translated(hook: &Hook, state: &mut State) -> Result<Vec<Value>> {
    if hook.kind() == HookKind::SubagentStart
        && let (Some(id), Some(kind)) = (hook.0["agent_id"].as_str(), hook.0["agent_type"].as_str())
    {
        state.subagents.insert(id.to_owned(), kind.to_owned());
    }
    if matches!(
        hook.kind(),
        HookKind::SessionStart | HookKind::UserPromptSubmit
    ) {
        state.stopped = false;
    }
    if hook.kind() == HookKind::SessionEnd {
        state.stopped = true;
        state.subagents.clear();
    }
    let Some(caller) = hook.caller(state) else {
        return Ok(vec![]);
    };
    let at = crate::yard::now().map_err(super::message)?.to_string();
    let base = |kind: &str| {
        envelope(
            kind,
            (hook.session(), hook.dir()),
            caller.agent.as_ref(),
            hook.0["turn_id"].as_str(),
            &at,
        )
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
    let kind = match hook.kind() {
        HookKind::UserPromptSubmit => "turn.start",
        HookKind::Stop | HookKind::Interrupt => "turn.end",
        HookKind::SubagentStart if caller.agent.is_some() => "subagent.start",
        HookKind::SubagentStop if caller.agent.is_some() => "subagent.end",
        HookKind::PostCompact => "compaction",
        HookKind::PreToolUse => "tool.start",
        HookKind::PostToolUse => "tool.end",
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
    if hook.kind() == HookKind::SubagentStop
        && let Some(id) = hook.0["agent_id"].as_str()
    {
        state.subagents.remove(id);
    }
    Ok(events)
}

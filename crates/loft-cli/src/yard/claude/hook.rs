//! One hook object of Claude Code's, as `loft yard claude hook` reads it: the session and the
//! directory it names, checked before anything is read or written for it, and the agent it
//! names where a subagent runs it.

use std::path::{Path, PathBuf};

use serde_json::Value;
use yardlog::identity::is_session_id;

use crate::yard::args::Runner;

/// The type a subagent is named by where its hook names none, as the Loft mod names it.
const SUBAGENT: &str = "subagent";

pub struct Hook {
    raw: Value,
    session: String,
    dir: PathBuf,
}

impl Hook {
    /// `raw` as a hook, refused where it names no event, a session that is no session id, a
    /// directory that is not absolute or a subagent that is no id.
    pub fn read(raw: Value) -> Result<Self, String> {
        if raw["hook_event_name"].as_str().is_none_or(str::is_empty) {
            return Err("the hook names no `hook_event_name`".to_owned());
        }
        let session = raw["session_id"].as_str().unwrap_or_default().to_owned();
        if !is_session_id(&session) {
            return Err(format!(
                "`session_id` is no session id: {}",
                raw["session_id"]
            ));
        }
        let dir = PathBuf::from(raw["cwd"].as_str().unwrap_or_default());
        if !dir.is_absolute() {
            return Err(format!("`cwd` is no absolute directory: {}", raw["cwd"]));
        }
        if raw.get("agent_id").is_some()
            && raw["agent_id"].as_str().is_none_or(|id| !is_session_id(id))
        {
            return Err(format!("`agent_id` is no agent id: {}", raw["agent_id"]));
        }
        Ok(Self { raw, session, dir })
    }

    /// The hook's event, such as `SessionStart`.
    pub fn event(&self) -> &str {
        self.raw["hook_event_name"].as_str().unwrap_or_default()
    }

    /// A text field of the hook, where it has one.
    pub fn field(&self, name: &str) -> Option<&str> {
        self.raw[name].as_str()
    }

    pub fn session(&self) -> &str {
        &self.session
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The agent that runs the hook, as a `loft yard` command names it.
    pub fn runner(&self) -> Runner {
        let agent = self.field("agent_id").map(ToOwned::to_owned);
        let agent_type = agent
            .as_ref()
            .map(|_| self.field("agent_type").unwrap_or(SUBAGENT).to_owned());
        Runner {
            host: yardlog::act::CLAUDE_CODE.to_owned(),
            session: Some(self.session.clone()),
            agent,
            agent_type,
        }
    }
}

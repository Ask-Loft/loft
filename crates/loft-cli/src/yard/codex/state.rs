//! Machine-local adapter state, atomically replaced under a per-session lock.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use loft_ports::sync::{read_if_present, replace_synced};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use yardlog_store::home::Home;
use yardlog_store::lock::{self, Locked};

use super::Result;
use super::translate::Hook;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub events: Vec<Value>,
    pub dropped: u64,
    pub in_flight: Vec<Value>,
    pub in_flight_dropped: u64,
    pub planning: Option<bool>,
    pub stopped: bool,
    pub subagents: BTreeMap<String, String>,
    pub ended_tasks: BTreeSet<String>,
    pub usage: super::usage::Usage,
    pub notifications: super::notification_state::Ledger,
}

impl State {
    pub fn keep(&mut self, events: Vec<Value>) {
        self.events.extend(events);
        self.trim();
    }

    pub fn trim(&mut self) {
        let over = self.events.len().saturating_sub(1000);
        self.events.drain(..over);
        self.dropped += over as u64;
    }
}

pub struct Files(pub PathBuf);

impl Files {
    pub fn of(hook: &Hook) -> Result<Self> {
        let root = std::env::var_os("PLUGIN_DATA")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| Home::from_env().map(|home| home.dir().join("codex")))
            .ok_or("no machine home for the Codex adapter")?;
        if !root.is_absolute() {
            return Err("PLUGIN_DATA must be an absolute directory".to_owned());
        }
        Ok(Self(root.join(hook.session())))
    }

    pub fn lock(&self) -> Result<Locked> {
        lock::lock(&self.0.join("state.lock")).map_err(|error| error.to_string())
    }

    pub fn read(&self) -> Result<State> {
        read_if_present(&self.0.join("state.json"))
            .map_err(|e| e.to_string())?
            .map_or_else(
                || Ok(State::default()),
                |bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
            )
    }

    pub fn write(&self, state: &State) -> Result<()> {
        fs::create_dir_all(&self.0).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(state).map_err(|e| e.to_string())?;
        replace_synced(&self.0.join("state.json"), &bytes, true).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use serde_json::json;

    #[test]
    fn the_codex_outbox_drops_the_oldest_events_past_its_limit() {
        let mut state = State::default();
        state.keep((0..1003).map(|n| json!({"n":n})).collect());
        assert_eq!(state.events.len(), 1000);
        assert_eq!(state.events[0]["n"], 3);
        assert_eq!(state.dropped, 3);
    }
}

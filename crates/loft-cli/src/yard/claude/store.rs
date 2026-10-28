//! What the Claude Yard hook keeps on the machine between hooks, in `~/.yard/claude/`: each file
//! one JSON object, changed under the directory's lock and replaced whole, and never anything
//! under the working tree.

use std::fs;
use std::path::PathBuf;

use loft_ports::sync::{read_if_present, replace_synced};
use serde_json::{Map, Value};
use yardlog_store::home::Home;
use yardlog_store::lock;

/// The sessions handed on by the Claude Code process that ended them, by its process id.
pub const HANDOFFS: &str = "handoffs.json";

/// Each session's last permission mode, whether it is `plan`, by its session id.
pub const MODES: &str = "modes.json";

/// `~/.yard/claude/`, where a home is known.
pub struct Store(PathBuf);

impl Store {
    pub fn of_home() -> Option<Self> {
        Home::from_env().map(|home| Self(home.dir().join("claude")))
    }

    /// Whether `file` is there.
    pub fn holds(&self, file: &str) -> bool {
        self.0.join(file).is_file()
    }

    /// Changes the object of `file` under the lock, and writes it only where the change left it
    /// different; what `change` gives.
    pub fn update<R>(
        &self,
        file: &str,
        change: impl FnOnce(&mut Map<String, Value>) -> R,
    ) -> Result<R, String> {
        fs::create_dir_all(&self.0).map_err(|error| error.to_string())?;
        let _locked = lock::lock(&self.0.join("lock")).map_err(|error| error.to_string())?;
        let path = self.0.join(file);
        let held = read_if_present(&path).map_err(|error| error.to_string())?;
        let mut map = match held.as_deref().map(serde_json::from_slice::<Value>) {
            Some(Ok(Value::Object(map))) => map,
            _ => Map::new(),
        };
        let before = map.clone();
        let result = change(&mut map);
        if map != before {
            let bytes = super::super::home_json::bytes(&map);
            replace_synced(&path, &bytes, true).map_err(|error| error.to_string())?;
        }
        Ok(result)
    }
}

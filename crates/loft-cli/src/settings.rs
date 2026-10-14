//! What `loft` remembers between runs for a user on a machine: one JSON file for each concern in
//! `loft/` under the user's configuration directory. A file that is missing or does not parse
//! reads as empty, so a damaged setting never stops a command.

use std::fmt;
use std::path::PathBuf;
use std::{env, fs, io};

use serde_json::{Value, json};

/// A settings file that could not be written.
#[derive(Debug)]
pub struct SettingsError {
    path: Option<PathBuf>,
    error: io::Error,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(
                formatter,
                "{}: cannot write the setting: {}",
                path.display(),
                self.error
            ),
            None => write!(
                formatter,
                "no configuration directory: {} is not set",
                if cfg!(windows) { "APPDATA" } else { "HOME" }
            ),
        }
    }
}

/// The directory `loft` keeps its settings in: `loft/` under `%APPDATA%` on Windows, under
/// `~/Library/Application Support` on macOS, and under `$XDG_CONFIG_HOME` or `~/.config`
/// elsewhere; none when the platform's variable is not set.
pub fn directory() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        variable("APPDATA")
    } else if cfg!(target_os = "macos") {
        variable("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        variable("XDG_CONFIG_HOME")
            .filter(|path| path.is_absolute())
            .or_else(|| variable("HOME").map(|home| home.join(".config")))
    };
    base.map(|base| base.join("loft"))
}

/// The settings file `name`, if there is a directory for it.
pub fn path(name: &str) -> Option<PathBuf> {
    directory().map(|directory| directory.join(name))
}

/// The contents of the settings file `name`: an empty object when it is missing, unreadable or
/// not a JSON object.
pub fn read(name: &str) -> Value {
    path(name)
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// Writes the settings file `name` whole, through a temporary file renamed over it, so that a
/// reader never sees half of it.
///
/// # Errors
///
/// There is no configuration directory, or the file cannot be written.
pub fn write(name: &str, value: &Value) -> Result<(), SettingsError> {
    let path = path(name).ok_or(SettingsError {
        path: None,
        error: io::Error::from(io::ErrorKind::NotFound),
    })?;
    let fail = |error| SettingsError {
        path: Some(path.clone()),
        error,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(fail)?;
    }
    let mut temporary = path.clone().into_os_string();
    temporary.push(".new");
    let bytes = serde_json::to_vec_pretty(value).expect("a JSON value serializes");
    fs::write(&temporary, bytes).map_err(fail)?;
    fs::rename(&temporary, &path).map_err(fail)
}

/// The value of the environment variable `name` as a path, when it is set and not empty.
fn variable(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

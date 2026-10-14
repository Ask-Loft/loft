//! What `loft` remembers between runs for a user on a machine: one JSON file for each concern in
//! `loft/` under the user's configuration directory. A file that is missing or does not parse
//! reads as empty, so a damaged setting never stops a command.

use std::path::{Path, PathBuf};
use std::{fmt, fs, io, process};

use serde_json::{Value, json};

use crate::environment;

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
fn directory() -> Option<PathBuf> {
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
fn path(name: &str) -> Option<PathBuf> {
    directory().map(|directory| directory.join(name))
}

/// The contents of the settings file `name`: an empty object when it is missing, unreadable or
/// not a JSON object.
pub fn read(name: &str) -> Value {
    path(name).map_or_else(|| json!({}), |path| read_at(&path))
}

/// The contents of the JSON file at `path`, read as [`read`] reads a settings file.
pub fn read_at(path: &Path) -> Value {
    fs::read(path)
        .ok()
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
    write_at(&path, value)
}

/// Writes `value` to the JSON file at `path` as [`write`] writes a settings file, creating its
/// directory. The temporary file is named for this process, so two runs at once never write
/// into the same one.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_at(path: &Path, value: &Value) -> Result<(), SettingsError> {
    let fail = |error| SettingsError {
        path: Some(path.to_owned()),
        error,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(fail)?;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = path.with_file_name(format!(".{name}.loft-{}", process::id()));
    let bytes = serde_json::to_vec_pretty(value).expect("a JSON value serializes");
    fs::write(&temporary, bytes).map_err(fail)?;
    fs::rename(&temporary, path).map_err(fail)
}

/// The value of the environment variable `name` as a path, when it is set and not empty.
fn variable(name: &str) -> Option<PathBuf> {
    environment::value(name).map(PathBuf::from)
}

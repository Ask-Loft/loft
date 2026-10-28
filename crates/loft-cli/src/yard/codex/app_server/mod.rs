//! Accounting-only port to an already running local Codex app server.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::Result;

#[derive(Debug, PartialEq)]
pub enum Notice {
    Usage {
        thread: String,
        turn: String,
        usage: Value,
    },
    End {
        thread: String,
        turn: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallFailure {
    Refused,
    Unavailable,
}

pub fn endpoint(
    explicit: Option<&Path>,
    codex_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        if !path.is_absolute() {
            return Err("--app-server-socket must be an absolute path".to_owned());
        }
        return Ok(Some(path.to_owned()));
    }
    Ok(codex_home
        .or_else(|| home.map(|path| path.join(".codex")))
        .filter(|path| path.is_absolute())
        .map(|path| path.join("app-server-control/app-server-control.sock")))
}

mod connection;
mod socket;
pub use connection::Connection;

pub mod notifications;

#[cfg(all(test, unix))]
mod tests;

#[cfg(all(test, unix))]
mod contract;

#[cfg(all(test, unix))]
mod account_tests;

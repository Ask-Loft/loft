//! How one page server learns of another: `~/.yard/server.json` names the running one, which
//! holds the lock on `~/.yard/server.lock` while it runs; and a newer one asks it to stop through
//! `~/.yard/stop.json`, which names the newer `loft`'s version. No connection is opened to learn
//! either.

use std::cmp::Ordering;

use serde_json::{Value, json};
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::lock;

use crate::update_check;
use crate::version::VERSION;
use crate::yard::failure::Failure;
use crate::yard::page::SERVER_LOCK;

/// The page server `~/.yard/server.json` names, while it holds the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    pub port: u16,
    pub version: String,
}

/// The page server that runs, where one does: the lock on `~/.yard/server.lock` is held and
/// `~/.yard/server.json` names it.
pub fn running(home: &Home) -> Result<Option<Running>, Failure> {
    let held = home.dir().join(SERVER_LOCK);
    if !held.is_file() || !lock::is_held(&held)? {
        return Ok(None);
    }
    let Some(bytes) = home.read(HomeFile::Server)? else {
        return Ok(None);
    };
    let Ok(server) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(None);
    };
    let port = server["port"]
        .as_u64()
        .and_then(|port| u16::try_from(port).ok());
    let version = server["version"].as_str().map(ToOwned::to_owned);
    Ok(port
        .zip(version)
        .map(|(port, version)| Running { port, version }))
}

/// How `version` compares with this `loft`'s: a version that does not read counts as older.
pub fn against_ours(version: &str) -> Ordering {
    let ours = update_check::parse(VERSION);
    match (update_check::parse(version), ours) {
        (Some(theirs), Some(ours)) => theirs.cmp(&ours),
        _ => Ordering::Less,
    }
}

/// Asks the running page server to stop, naming this `loft`'s version.
pub fn request_stop(home: &Home) -> Result<(), Failure> {
    let request = json!({ "version": VERSION, "pid": std::process::id() });
    home.write(HomeFile::Stop, request.to_string().as_bytes())?;
    Ok(())
}

/// Whether `~/.yard/stop.json` holds a request made by a `loft` newer than this one.
pub fn asked_to_stop(home: &Home) -> bool {
    let Ok(Some(bytes)) = home.read(HomeFile::Stop) else {
        return false;
    };
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|request| request["version"].as_str().map(against_ours))
        == Some(Ordering::Greater)
}

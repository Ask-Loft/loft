//! How one page server learns of another: `~/.yard/server.json` names the running one, which
//! holds the lock on `~/.yard/server.lock` while it runs; and a newer one asks it to stop through
//! `~/.yard/stop.json`, which names the newer `loft`'s version. No connection is opened to learn
//! either.

use std::cmp::Ordering;

use serde_json::{Value, json};
use yardlog_store::home::{Home, HomeFile};

use crate::update_check;
use crate::version::VERSION;
use crate::yard::failure::Failure;

pub use crate::yard::page::{Running, running};

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

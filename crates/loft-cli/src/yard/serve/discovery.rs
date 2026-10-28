//! How one page server learns of another: `~/.yard/server.json` names the running one, which
//! holds the lock on `~/.yard/server.lock` while it runs; and a newer one, or one run to replace
//! it, asks it to stop through `~/.yard/stop.json`, which names the asking `loft`'s version and
//! whether it replaces the running one whatever its version. No connection is opened to learn
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

/// Asks the running page server to stop, naming this `loft`'s version, and whether it is to
/// stop whatever its own version, to be replaced.
pub fn request_stop(home: &Home, replace: bool) -> Result<(), Failure> {
    let mut request = json!({ "version": VERSION, "pid": std::process::id() });
    if replace {
        request["replace"] = json!(true);
    }
    home.write(HomeFile::Stop, request.to_string().as_bytes())?;
    Ok(())
}

/// Whether `~/.yard/stop.json` holds a request that replaces this page server, or one made by a
/// `loft` newer than this one.
pub fn asked_to_stop(home: &Home) -> bool {
    let Ok(Some(bytes)) = home.read(HomeFile::Stop) else {
        return false;
    };
    let Ok(request) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    request["replace"] == true
        || request["version"].as_str().map(against_ours) == Some(Ordering::Greater)
}

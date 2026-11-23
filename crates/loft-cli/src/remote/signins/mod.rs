//! The person's sign-ins, kept in `loft/remotes.json` under the user's configuration directory,
//! or in the file a test names: a JSON object whose `remotes` maps each remote's address to its
//! sign-in. The file is written whole, with permission for its owner alone, through a file beside
//! it and a rename, and a refresh holds a lock beside it so that two processes never rotate one
//! refresh token at once.

mod kept;
mod refresh;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::Duration;

use loft_ports::sync::replace_synced;
use loftspec::publish::PublishedAt as Time;
use serde_json::{Map, Value, json};
use yardlog_store::lock::{self, Locked};

use super::RemoteError;
use crate::settings;

pub use kept::SignIn;

/// The file's name in the configuration directory.
const FILE: &str = "remotes.json";

/// How close to its expiry an access token is refreshed before a call.
const MARGIN: u64 = 5 * 60;

/// The person's `loft/remotes.json`, in the user's configuration directory.
///
/// # Errors
///
/// There is no configuration directory.
pub fn file() -> Result<PathBuf, RemoteError> {
    settings::path(FILE).ok_or_else(|| {
        RemoteError::File("no configuration directory for loft/remotes.json".to_owned())
    })
}

/// The time now in seconds since the Unix epoch, or the epoch where the system clock does not
/// read.
fn now() -> u64 {
    super::now().unwrap_or(0)
}

/// Every entry `file` holds, by address, as it was written: none for a file that is missing or
/// does not read. An entry this `loft` does not read is kept as it is when the file is written
/// again.
fn entries(file: &Path) -> Map<String, Value> {
    settings::read_at(file)["remotes"]
        .as_object()
        .cloned()
        .unwrap_or_default()
}

/// Writes `sign_ins` as the whole of `file`, with permission for its owner alone, through a file
/// beside it and a rename. The lock beside it has made the folder.
fn write(file: &Path, sign_ins: &Map<String, Value>) -> Result<(), RemoteError> {
    let bytes = serde_json::to_vec_pretty(&json!({ "remotes": sign_ins }))
        .expect("a JSON value serializes");
    replace_synced(file, &bytes, true).map_err(|error| RemoteError::File(error.to_string()))
}

/// The lock beside `file`, held while it is read and written again: the file's name with
/// `.lock` appended.
fn lock(file: &Path) -> Result<Locked, RemoteError> {
    let mut name = file.as_os_str().to_owned();
    name.push(".lock");
    lock::lock(Path::new(&name)).map_err(|error| RemoteError::File(error.to_string()))
}

/// Keeps `sign_in` for `address` in `file`, in place of any it held for it.
///
/// # Errors
///
/// The file cannot be written.
pub fn save(file: &Path, address: &str, sign_in: &SignIn) -> Result<(), RemoteError> {
    let _lock = lock(file)?;
    let mut sign_ins = entries(file);
    sign_ins.insert(address.to_owned(), json!(sign_in));
    write(file, &sign_ins)
}

/// Forgets the sign-in for `address` in `file`, and says whether it held one. A file that holds
/// none is left as it is, and a missing file is not created.
///
/// # Errors
///
/// The file cannot be written.
pub fn forget(file: &Path, address: &str) -> Result<bool, RemoteError> {
    if !file.exists() {
        return Ok(false);
    }
    let _lock = lock(file)?;
    let mut sign_ins = entries(file);
    if sign_ins.remove(address).is_none() {
        return Ok(false);
    }
    write(file, &sign_ins).map(|()| true)
}

/// The sign-in that `file` keeps for `address`, refreshed first if its access token expires
/// within five minutes. The refresh runs under the file's lock, after the file is read again, so
/// that a process that waited for another uses the successor the other kept; the successor is
/// written before it is used.
///
/// # Errors
///
/// The file holds no sign-in for `address`, or one only for a protected resource on another
/// origin or for a resource that reads as no address, or the refresh is refused or cannot be
/// made.
pub fn live(file: &Path, address: &str, timeout: Duration) -> Result<SignIn, RemoteError> {
    let held = kept::find(&entries(file), address)?;
    if !held.expiring(now()) {
        return Ok(held);
    }
    let _lock = lock(file)?;
    refresh::refreshed(file, address, timeout)
}

/// The sign-in that the person's `loft/remotes.json` keeps for `address`, as [`live`] gives it:
/// none where there is no configuration directory to hold the file, so that a read finds no
/// sign-in where a write still refuses.
///
/// # Errors
///
/// As [`live`], and no sign-in where there is no configuration directory.
pub fn held(address: &str, timeout: Duration) -> Result<SignIn, RemoteError> {
    let file = settings::path(FILE).ok_or_else(|| RemoteError::NoSignIn {
        address: address.to_owned(),
    })?;
    live(&file, address, timeout)
}

/// The time `expires_in` seconds from now, as a sign-in keeps it.
pub fn expires_at(expires_in: u64) -> String {
    Time::from_unix_seconds(now().saturating_add(expires_in))
        .map_or_else(|_| String::new(), |time| time.to_string())
}

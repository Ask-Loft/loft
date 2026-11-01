//! The person's sign-ins, kept in `loft/remotes.json` under the user's configuration directory:
//! a JSON object whose `remotes` maps each remote's address to its sign-in. The file is written
//! whole, with permission for its owner alone, through a file beside it and a rename, and a
//! refresh holds a lock beside it so that two processes never rotate one refresh token at once.

use std::path::PathBuf;
use std::str::FromStr as _;
use std::time::Duration;

use loft_ports::sync::replace_synced;
use loftspec::publish::PublishedAt as Time;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use yardlog_store::lock::{self, Locked};

use super::{RemoteError, signin};
use crate::settings;

/// The file's name in the configuration directory.
const FILE: &str = "remotes.json";

/// How close to its expiry an access token is refreshed before a call.
const MARGIN: u64 = 5 * 60;

/// A sign-in to one remote: the token endpoint and resource it refreshes with, the client it was
/// issued to, its tokens, and when its access token expires, in UTC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    pub token_endpoint: String,
    pub resource: String,
    pub client_id: String,
    pub access_token: String,
    pub expires_at: String,
    pub refresh_token: String,
}

impl SignIn {
    /// Whether its access token expires within [`MARGIN`] of `now`, or at a time that does not
    /// read.
    fn expiring(&self, now: u64) -> bool {
        Time::from_str(&self.expires_at)
            .map_or(true, |expires| expires.unix_seconds() <= now + MARGIN)
    }
}

/// The time now in seconds since the Unix epoch, or the epoch where the system clock does not
/// read.
fn now() -> u64 {
    super::now().unwrap_or(0)
}

/// The file's path, if there is a configuration directory.
fn path() -> Result<PathBuf, RemoteError> {
    settings::path(FILE).ok_or_else(|| {
        RemoteError::File("no configuration directory for loft/remotes.json".to_owned())
    })
}

/// Every entry the file holds, by address, as it was written: none for a file that is missing
/// or does not read. An entry this `loft` does not read is kept as it is when the file is
/// written again.
fn entries() -> Result<Map<String, Value>, RemoteError> {
    let file = settings::read_at(&path()?);
    Ok(file["remotes"].as_object().cloned().unwrap_or_default())
}

/// The sign-in an entry holds, if it reads as one.
fn sign_in(entry: &Value) -> Option<SignIn> {
    serde_json::from_value(entry.clone()).ok()
}

/// Writes `sign_ins` as the whole file, with permission for its owner alone, through a file
/// beside it and a rename. The lock beside it has made the folder.
fn write(sign_ins: &Map<String, Value>) -> Result<(), RemoteError> {
    let bytes = serde_json::to_vec_pretty(&json!({ "remotes": sign_ins }))
        .expect("a JSON value serializes");
    replace_synced(&path()?, &bytes, true).map_err(|error| RemoteError::File(error.to_string()))
}

/// The lock beside the file, held while it is read and written again.
fn lock() -> Result<Locked, RemoteError> {
    lock::lock(&path()?.with_file_name(format!("{FILE}.lock")))
        .map_err(|error| RemoteError::File(error.to_string()))
}

/// Keeps `sign_in` for `address`, in place of any the file held for it.
///
/// # Errors
///
/// The file cannot be written.
pub fn save(address: &str, sign_in: &SignIn) -> Result<(), RemoteError> {
    let _lock = lock()?;
    let mut sign_ins = entries()?;
    sign_ins.insert(address.to_owned(), json!(sign_in));
    write(&sign_ins)
}

/// Forgets the sign-in for `address`, and says whether the file held one. A file that holds
/// none is left as it is, and a missing file is not created.
///
/// # Errors
///
/// The file cannot be written.
pub fn forget(address: &str) -> Result<bool, RemoteError> {
    if !path()?.exists() {
        return Ok(false);
    }
    let _lock = lock()?;
    let mut sign_ins = entries()?;
    if sign_ins.remove(address).is_none() {
        return Ok(false);
    }
    write(&sign_ins).map(|()| true)
}

/// The sign-in for `address`, refreshed first if its access token expires within five minutes.
/// The refresh runs under the file's lock, after the file is read again, so that a process that
/// waited for another uses the successor the other kept; the successor is written before it is
/// used.
///
/// # Errors
///
/// The file holds no sign-in for `address`, or the refresh is refused or cannot be made.
pub fn live(address: &str, timeout: Duration) -> Result<SignIn, RemoteError> {
    let missing = || RemoteError::NoSignIn {
        address: address.to_owned(),
    };
    let held = entries()?
        .get(address)
        .and_then(sign_in)
        .ok_or_else(missing)?;
    if !held.expiring(now()) {
        return Ok(held);
    }
    let _lock = lock()?;
    let mut sign_ins = entries()?;
    let held = sign_ins
        .get(address)
        .and_then(sign_in)
        .ok_or_else(missing)?;
    if !held.expiring(now()) {
        return Ok(held);
    }
    let fresh = signin::refresh(address, &held, timeout)?;
    sign_ins.insert(address.to_owned(), json!(fresh));
    write(&sign_ins)?;
    Ok(fresh)
}

/// The time `expires_in` seconds from now, as a sign-in keeps it.
pub fn expires_at(expires_in: u64) -> String {
    Time::from_unix_seconds(now().saturating_add(expires_in))
        .map_or_else(|_| String::new(), |time| time.to_string())
}

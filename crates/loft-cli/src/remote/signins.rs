//! The person's sign-ins, kept in `loft/remotes.json` under the user's configuration directory:
//! a JSON object whose `remotes` maps each remote's address to its sign-in. The file is written
//! whole, with permission for its owner alone, through a file beside it and a rename, and a
//! refresh holds a lock beside it so that two processes never rotate one refresh token at once.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use std::process;
use std::str::FromStr as _;
use std::time::Duration;

use loftspec::publish::PublishedAt as Time;
use loftspec_store::clock::{Clock as _, SystemClock};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

/// The file's path, if there is a configuration directory.
fn path() -> Result<PathBuf, RemoteError> {
    settings::path(FILE).ok_or_else(|| {
        RemoteError::File("no configuration directory for loft/remotes.json".to_owned())
    })
}

/// Every sign-in the file holds, by address: none for a file that is missing or does not read.
///
/// # Errors
///
/// There is no configuration directory.
pub fn read() -> Result<BTreeMap<String, SignIn>, RemoteError> {
    let file = settings::read_at(&path()?);
    Ok(file["remotes"]
        .as_object()
        .map(|remotes| {
            remotes
                .iter()
                .filter_map(|(address, entry)| {
                    Some((address.clone(), serde_json::from_value(entry.clone()).ok()?))
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Writes `sign_ins` as the whole file, with permission for its owner alone, through a file
/// beside it and a rename.
fn write(sign_ins: &BTreeMap<String, SignIn>) -> Result<(), RemoteError> {
    let path = path()?;
    let fail = |error: std::io::Error| RemoteError::File(format!("{}: {error}", path.display()));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(fail)?;
    }
    let value: Value = json!({ "remotes": sign_ins });
    let bytes = serde_json::to_vec_pretty(&value).expect("a JSON value serializes");
    let temporary = path.with_file_name(format!(".{FILE}.loft-{}", process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&temporary).map_err(fail)?;
    file.write_all(&bytes).map_err(fail)?;
    drop(file);
    fs::rename(&temporary, &path).map_err(fail)
}

/// The lock beside the file, held while it is read and written again.
fn lock() -> Result<File, RemoteError> {
    let path = path()?.with_file_name(format!("{FILE}.lock"));
    let fail = |error: std::io::Error| RemoteError::File(format!("{}: {error}", path.display()));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(fail)?;
    }
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(fail)?;
    file.lock().map_err(fail)?;
    Ok(file)
}

/// Keeps `sign_in` for `address`, in place of any the file held for it.
///
/// # Errors
///
/// The file cannot be written.
pub fn save(address: &str, sign_in: SignIn) -> Result<(), RemoteError> {
    let _lock = lock()?;
    let mut sign_ins = read()?;
    sign_ins.insert(address.to_owned(), sign_in);
    write(&sign_ins)
}

/// Forgets the sign-in for `address`, and says whether the file held one. A file that holds
/// none is left as it is, and a missing file is not created.
///
/// # Errors
///
/// The file cannot be written.
pub fn forget(address: &str) -> Result<bool, RemoteError> {
    let _lock = lock()?;
    let mut sign_ins = read()?;
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
    let clock = SystemClock::from_env();
    let now = || clock.now().unwrap_or(0);
    let missing = || RemoteError::NoSignIn {
        address: address.to_owned(),
    };
    let held = read()?.remove(address).ok_or_else(missing)?;
    if !held.expiring(now()) {
        return Ok(held);
    }
    let _lock = lock()?;
    let mut sign_ins = read()?;
    let held = sign_ins.remove(address).ok_or_else(missing)?;
    if !held.expiring(now()) {
        return Ok(held);
    }
    let fresh = signin::refresh(address, &held, timeout)?;
    sign_ins.insert(address.to_owned(), fresh.clone());
    write(&sign_ins)?;
    Ok(fresh)
}

/// The time `expires_in` seconds from now, as a sign-in keeps it.
pub fn expires_at(expires_in: u64) -> String {
    let now = SystemClock::from_env().now().unwrap_or(0);
    Time::from_unix_seconds(now.saturating_add(expires_in))
        .map_or_else(|_| String::new(), |time| time.to_string())
}

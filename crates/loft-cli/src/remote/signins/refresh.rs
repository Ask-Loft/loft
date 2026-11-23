//! The refresh of a kept sign-in, under the lock beside `loft/remotes.json`: the file is read
//! again, so that a process that waited for another uses the successor the other kept, and the
//! successor is written before the access token it came with is used.

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::kept::{self, SignIn};
use crate::remote::{RemoteError, signin};

/// The sign-in that `file` keeps for `address`, as the file holds it now, refreshed where its
/// access token expires within five minutes. The caller holds the lock beside `file`.
///
/// # Errors
///
/// The file holds no sign-in for `address`, or one only for a protected resource on another
/// origin or for a resource that reads as no address, or the refresh is refused or cannot be
/// made, or its successor cannot be written.
pub(super) fn refreshed(
    file: &Path,
    address: &str,
    timeout: Duration,
) -> Result<SignIn, RemoteError> {
    let mut sign_ins = super::entries(file);
    let held = kept::find(&sign_ins, address)?;
    if !held.expiring(super::now()) {
        return Ok(held);
    }
    let fresh = signin::refresh(address, &held, timeout)?;
    sign_ins.insert(address.to_owned(), json!(fresh));
    super::write(file, &sign_ins)?;
    Ok(fresh)
}

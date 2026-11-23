//! The refresh of a kept sign-in, under the lock beside `loft/remotes.json`: the file is read
//! again, so that a process that waited for another uses the successor the other kept, and the
//! successor is written before the access token it came with is used. A refresh names one
//! protected resource and keeps every member of the sign-in that it does not replace. For a
//! sign-in over companion resources, the resource of each refresh is recorded before it is sent
//! and the record removed with its answer, and a recorded refresh is repeated before any other: a
//! lost answer, followed by a refresh for another resource with the rotated refresh token, would
//! end the whole sign-in. An answer that is lost, or arrives and does not read, may follow a
//! rotation, so it leaves the record in place.

use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value};

use super::kept::{self, Kept, SignIn};
use crate::remote::RemoteError;
use crate::remote::signin::{self, Issued, TokenError};

/// The OAuth error that refuses a refresh for a protected resource that the sign-in does not
/// hold, or that the authorization server does not serve (RFC 8707).
const INVALID_TARGET: &str = "invalid_target";

/// What a refresh under the lock reads and writes: the file, the address whose sign-in it keeps,
/// and the time limit of a call.
pub(super) struct Run<'a> {
    pub(super) file: &'a Path,
    pub(super) address: &'a str,
    pub(super) timeout: Duration,
}

/// The access token for `resource`, or for the remote's own protected resource where it is none,
/// as the file holds it now, once a recorded refresh is repeated and the token is refreshed where
/// it needs to be. A repeat that cannot reach the token endpoint leaves its record, and the token
/// is still given where it expires later than five minutes ahead, since giving it presents no
/// refresh token. The caller holds the lock beside the file.
///
/// # Errors
///
/// The file holds no sign-in for the address, or one only for a protected resource on another
/// origin, or one whose record does not read or that does not cover `resource`; or the repeat or
/// the refresh is refused or cannot be made, or what it issued cannot be written.
pub(super) fn refreshed(run: &Run, resource: Option<&str>) -> Result<String, RemoteError> {
    let mut entries = super::entries(run.file);
    let mut held = Kept::find(&entries, run.address)?;
    let wanted = held.wanted(run.address, resource)?;
    if let Some(recorded) = held.refreshing.clone() {
        match repeat(run, &mut entries, &held, &recorded) {
            Ok(()) => held = Kept::find(&entries, run.address)?,
            Err(error @ RemoteError::Unreachable { .. }) => {
                return held.fresh(&wanted, super::now()).ok_or(error);
            }
            Err(error) => return Err(error),
        }
    }
    if let Some(token) = held.live(&wanted, super::now()) {
        return Ok(token);
    }
    fresh(run, &mut entries, &held, &wanted)
}

/// Repeats the refresh for `recorded` that `held` records, with the refresh token it keeps, and
/// keeps what the repeat issues. Whatever the repeat's error, nothing is written, and the record
/// stays for the next refresh of the sign-in to repeat.
fn repeat(
    run: &Run,
    entries: &mut Map<String, Value>,
    held: &Kept,
    recorded: &str,
) -> Result<(), RemoteError> {
    let issued = send(run, &held.sign_in, recorded)
        .map_err(|error| failure(run, &held.sign_in, recorded, error))?;
    let own = &held.sign_in.resource;
    kept::keep_issued(entry(entries, run.address), own, recorded, &issued);
    super::write(run.file, entries)
}

/// Refreshes `held` for `wanted`, recording the refresh first for a sign-in that records each
/// one, and gives the access token it issues once it is kept. A refusal removes the record; any
/// other failure leaves it, and the tokens, as they were.
fn fresh(
    run: &Run,
    entries: &mut Map<String, Value>,
    held: &Kept,
    wanted: &str,
) -> Result<String, RemoteError> {
    if held.records {
        kept::record(entry(entries, run.address), wanted);
        super::write(run.file, entries)?;
    }
    let own = &held.sign_in.resource;
    match send(run, &held.sign_in, wanted) {
        Ok(issued) => {
            kept::keep_issued(entry(entries, run.address), own, wanted, &issued);
            super::write(run.file, entries)?;
            Ok(issued.access_token)
        }
        Err(error @ TokenError::Refused(_)) => {
            if held.records {
                kept::unrecord(entry(entries, run.address));
                super::write(run.file, entries)?;
            }
            Err(failure(run, &held.sign_in, wanted, error))
        }
        Err(error) => Err(failure(run, &held.sign_in, wanted, error)),
    }
}

/// A refresh of `sign_in` for the one protected resource `resource`.
fn send(run: &Run, sign_in: &SignIn, resource: &str) -> Result<Issued, TokenError> {
    signin::refresh(
        &sign_in.token_endpoint,
        &sign_in.client_id,
        &sign_in.refresh_token,
        resource,
        run.timeout,
    )
}

/// The error that `error`, the failure of a refresh of `sign_in` for `resource`, stops a call
/// with: a sign-in made for a former resource of the remote, where its own resource is refused
/// as `invalid_target`; one that does not cover a companion resource refused so; a lapsed one,
/// where the authorization server refuses otherwise or answers what does not read; and the token
/// endpoint unreachable otherwise.
fn failure(run: &Run, sign_in: &SignIn, resource: &str, error: TokenError) -> RemoteError {
    let address = run.address.to_owned();
    let resource = resource.to_owned();
    match error {
        TokenError::Refused(code) if code == INVALID_TARGET && resource == sign_in.resource => {
            RemoteError::FormerResource { address, resource }
        }
        TokenError::Refused(code) if code == INVALID_TARGET => {
            RemoteError::NotCovered { address, resource }
        }
        TokenError::Refused(error) => RemoteError::Lapsed { address, error },
        TokenError::Unreadable => RemoteError::Lapsed {
            address,
            error: TokenError::Unreadable.to_string(),
        },
        TokenError::Unreachable(reason) => RemoteError::Unreachable {
            address: sign_in.token_endpoint.clone(),
            reason,
        },
    }
}

/// The sign-in that `entries` keep for `address`, as the file holds it, to write into.
fn entry<'a>(entries: &'a mut Map<String, Value>, address: &str) -> &'a mut Value {
    entries
        .get_mut(address)
        .expect("the sign-in was just read from these entries")
}

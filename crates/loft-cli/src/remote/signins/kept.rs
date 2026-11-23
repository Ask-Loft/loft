//! A sign-in as `loft/remotes.json` keeps it, and the one an entry keeps for an address: only a
//! sign-in for a protected resource on the address's origin is one for that address.

use std::str::FromStr as _;

use loftspec::publish::PublishedAt as Time;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::MARGIN;
use crate::network;
use crate::remote::RemoteError;

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
    pub(super) fn expiring(&self, now: u64) -> bool {
        Time::from_str(&self.expires_at)
            .map_or(true, |expires| expires.unix_seconds() <= now + MARGIN)
    }
}

/// The sign-in that `entries`, the entries of `loft/remotes.json` by address, keep for
/// `address`, where it reads as one and its resource lies on the address's origin.
///
/// # Errors
///
/// There is no entry, or it does not read as a sign-in, or its resource reads as no `https` or
/// `http` address. Or it is one for a protected resource on another origin, which an earlier
/// `loft` could keep: it is no sign-in for the address and is neither refreshed nor sent, and its
/// error, unlike that of a missing one, names that resource.
pub(super) fn find(entries: &Map<String, Value>, address: &str) -> Result<SignIn, RemoteError> {
    let none = || RemoteError::NoSignIn {
        address: address.to_owned(),
    };
    let held = entries
        .get(address)
        .and_then(|entry| SignIn::deserialize(entry).ok())
        .ok_or_else(none)?;
    // A resource that reads as no address lies on no origin, so it is not on another one.
    let origin = network::origin(&held.resource).ok_or_else(none)?;
    if network::origin(address).as_ref() == Some(&origin) {
        Ok(held)
    } else {
        Err(RemoteError::SignInElsewhere {
            address: address.to_owned(),
            resource: held.resource,
        })
    }
}

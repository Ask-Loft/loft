//! Reaching a connected document's remote: its address, from the manifest's `remote` or
//! `LOFT_REMOTE_ENDPOINT`; the token a request carries, from `LOFT_REMOTE_TOKEN` or the person's
//! sign-in to that address in `loft/remotes.json`, refreshed before it expires; and the calls
//! themselves, through [`client::Client`]. A token is sent only in the `Authorization` header of
//! a request to the remote, and printed nowhere.

pub mod browser;
pub mod client;
pub mod signin;
pub mod signins;
mod wire;

use std::fmt;
use std::time::Duration;

use crate::environment;
use crate::network;

pub use client::{Client, Refusal};

/// The variable that names the bearer token for one run, in place of a sign-in.
pub const TOKEN_ENV: &str = "LOFT_REMOTE_TOKEN";
/// The variable that replaces the 30 seconds a call to the remote may take.
pub const TIMEOUT_ENV: &str = "LOFT_REMOTE_TIMEOUT";
/// The variable that lets a run wait out a refusal the remote may lift.
pub const WAIT_ENV: &str = "LOFT_REMOTE_WAIT";

/// How long a call to the remote may take, unless `LOFT_REMOTE_TIMEOUT` says otherwise.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Why the remote could not be reached, or what it refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteError {
    /// The address has no sign-in, and `LOFT_REMOTE_TOKEN` is not set.
    NoSignIn { address: String },
    /// The address is neither `https` nor `http` on the loopback interface.
    Address(String),
    /// A variable that must be a positive whole number of seconds is not one.
    Variable { name: &'static str, value: String },
    /// The remote refused the connection, gave no answer, or one that does not read.
    Unreachable { address: String, reason: String },
    /// The remote answered with a refusal.
    Refused { address: String, refusal: Refusal },
    /// The authorization server refused to refresh the sign-in.
    Lapsed { address: String, error: String },
    /// A sign-in could not be made.
    SignIn { address: String, reason: String },
    /// `loft/remotes.json` could not be read or written.
    File(String),
}

impl fmt::Display for RemoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSignIn { address } => write!(
                formatter,
                "no sign-in to {address} in loft/remotes.json: `loft remote login` signs in, \
                 or {TOKEN_ENV} names a token for one run"
            ),
            Self::Address(address) => write!(
                formatter,
                "{address}: a remote's address is `https`, or `http` on the loopback interface"
            ),
            Self::Variable { name, value } => write!(
                formatter,
                "{name}={value}: not a positive whole number of seconds"
            ),
            Self::Unreachable { address, reason } => {
                write!(
                    formatter,
                    "{address}: the remote cannot be reached: {reason}"
                )
            }
            Self::Refused { address, refusal } => write!(formatter, "{address}: {refusal}"),
            Self::Lapsed { address, error } => write!(
                formatter,
                "the sign-in to {address} has lapsed ({error}): `loft remote login` signs in \
                 again"
            ),
            Self::SignIn { address, reason } => {
                write!(formatter, "cannot sign in to {address}: {reason}")
            }
            Self::File(reason) => formatter.write_str(reason),
        }
    }
}

/// `address` as a remote's address, if it may be contacted.
///
/// # Errors
///
/// The address is neither `https` nor `http` on the loopback interface.
pub fn checked(address: &str) -> Result<&str, RemoteError> {
    network::allowed(address)
        .map(|_| address)
        .ok_or_else(|| RemoteError::Address(address.to_owned()))
}

/// The value of `name` as a positive whole number of seconds, or none when it is not set.
///
/// # Errors
///
/// The variable is set to something else.
pub fn seconds(name: &'static str) -> Result<Option<u64>, RemoteError> {
    let Some(value) = environment::text(name) else {
        return Ok(None);
    };
    match value.parse::<u64>() {
        Ok(seconds) if seconds > 0 => Ok(Some(seconds)),
        _ => Err(RemoteError::Variable { name, value }),
    }
}

/// How long a call to the remote may take.
///
/// # Errors
///
/// `LOFT_REMOTE_TIMEOUT` is not a positive whole number of seconds.
pub fn timeout() -> Result<Duration, RemoteError> {
    Ok(seconds(TIMEOUT_ENV)?.map_or(TIMEOUT, Duration::from_secs))
}

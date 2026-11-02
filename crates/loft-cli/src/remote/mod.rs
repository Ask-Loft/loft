//! Reaching a connected document's remote: its address, from the manifest's `remote` or
//! `LOFT_REMOTE_ENDPOINT`; the token a request carries, from `LOFT_REMOTE_TOKEN` or the person's
//! sign-in to that address in `loft/remotes.json`, refreshed before it expires; and the calls
//! themselves, through [`client::Client`]. A token is sent only in the `Authorization` header of
//! a request to the remote, and printed nowhere.

pub mod browser;
pub mod client;
pub mod files;
pub mod head;
pub mod history;
pub mod signin;
pub mod signins;
mod wire;

use std::fmt;
use std::time::Duration;

use loft_ports::clock::{Clock as _, SystemClock};

use crate::environment;
use crate::network;

pub use client::{Client, Refusal};
pub use wire::sha256_hex;

/// The variable that names the remote's address for one run, in place of the manifest's.
pub const ENDPOINT_ENV: &str = "LOFT_REMOTE_ENDPOINT";
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
    /// The document is connected but no address names its remote.
    NoAddress { key: String },
    /// The address has no sign-in, and `LOFT_REMOTE_TOKEN` is not set.
    NoSignIn { address: String },
    /// The address is neither `https` nor `http` on the loopback interface.
    Address(String),
    /// The remote holds no document under the key.
    NotHeld { key: String },
    /// The remote holds no published state with the lock's publish sequence and state root.
    StateNotHeld { sequence: u64, state_root: String },
    /// A variable that must be a positive whole number of seconds is not one.
    Variable { name: &'static str, value: String },
    /// The remote refused the connection, gave no answer, or one that does not read.
    Unreachable { address: String, reason: String },
    /// The remote answered, but what it gave as `what` does not read.
    Unreadable { address: String, what: &'static str },
    /// The remote answered with a refusal.
    Refused { address: String, refusal: Refusal },
    /// The authorization server refused to refresh the sign-in.
    Lapsed { address: String, error: String },
    /// A sign-in could not be made.
    SignIn { address: String, reason: String },
    /// `loft/remotes.json` could not be read or written.
    File(String),
}

impl RemoteError {
    /// The remote's refusal, where it refused with the known code `code`.
    #[must_use]
    pub fn refusal(&self, code: &str) -> Option<&Refusal> {
        match self {
            Self::Refused { refusal, .. } if refusal.known() == Some(code) => Some(refusal),
            _ => None,
        }
    }
}

impl fmt::Display for RemoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAddress { key } => write!(
                formatter,
                "the document {key} is connected but names no remote: `remote` in \
                 spec/loftspec.yaml names it, or {ENDPOINT_ENV} for one run"
            ),
            Self::NoSignIn { address } => write!(
                formatter,
                "no sign-in to {address} in loft/remotes.json: `loft remote login` signs in, \
                 or {TOKEN_ENV} names a token for one run"
            ),
            Self::Address(address) => write!(
                formatter,
                "{address}: a remote's address is `https`, or `http` on the loopback interface"
            ),
            Self::NotHeld { key } => write!(
                formatter,
                "the remote holds no document {key}: `loft remote set <address>` connects the \
                 document to its remote"
            ),
            Self::StateNotHeld {
                sequence,
                state_root,
            } => write!(
                formatter,
                "the remote holds no published state of publish sequence {sequence}, state root \
                 {state_root}"
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
            Self::Unreadable { address, what } => {
                write!(formatter, "{address}: the remote's {what} does not read")
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
    Ok(time_limit(seconds(TIMEOUT_ENV)?))
}

/// How long a call may take: `seconds` where `LOFT_REMOTE_TIMEOUT` gives them, or else 30
/// seconds.
fn time_limit(seconds: Option<u64>) -> Duration {
    seconds.map_or(TIMEOUT, Duration::from_secs)
}

/// The time now in seconds since the Unix epoch, from the system clock alone: a token expires and
/// a `Retry-After` date falls in real time, whatever time `SOURCE_DATE_EPOCH` gives a publish.
pub fn now() -> Option<u64> {
    SystemClock::default().now().ok()
}

/// The client of a connected document's remote, whose manifest names `remote` and whose key is
/// `key`: at `LOFT_REMOTE_ENDPOINT` or the manifest's address, with `LOFT_REMOTE_TOKEN` or the
/// person's sign-in to that address, refreshed if it expires within five minutes. The timeout
/// variables are read before anything else, so a bad value opens no connection.
///
/// # Errors
///
/// A variable is not a whole number of seconds, there is no address or no sign-in, the address
/// may not be contacted, or a refresh is refused.
pub fn connected(remote: Option<&str>, key: &str) -> Result<Client, RemoteError> {
    let timeout = timeout()?;
    let wait = seconds(WAIT_ENV)?;
    let address = environment::text(ENDPOINT_ENV)
        .or_else(|| remote.map(str::to_owned))
        .ok_or_else(|| RemoteError::NoAddress {
            key: key.to_owned(),
        })?;
    checked(&address)?;
    let token = match environment::text(TOKEN_ENV) {
        Some(token) => token,
        None => signins::live(&address, timeout)?.access_token,
    };
    Ok(Client::new(&address, Some(token), timeout, wait))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::time_limit;

    #[test]
    fn a_call_may_take_30_seconds_unless_loft_remote_timeout_says_otherwise() {
        assert_eq!(time_limit(None), Duration::from_secs(30));
        assert_eq!(time_limit(Some(5)), Duration::from_secs(5));
    }
}

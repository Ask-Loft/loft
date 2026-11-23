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
mod said;
pub mod signin;
pub mod signins;
pub mod states;
pub mod visibility;
mod whose;
mod wire;

use std::fmt;
use std::time::Duration;

use loft_ports::clock::{Clock as _, SystemClock};

use crate::environment;
use crate::network;

pub use client::{Client, Refusal};
pub use said::Remedy;
pub use whose::Whose;
pub use wire::sha256_hex;

/// The variable that names the remote's address for one run, in place of the manifest's.
pub const ENDPOINT_ENV: &str = "LOFT_REMOTE_ENDPOINT";
/// The variable that names the bearer token for one run, in place of a sign-in.
pub const TOKEN_ENV: &str = "LOFT_REMOTE_TOKEN";
/// The variable that replaces the 30 seconds a call to the remote may take.
pub const TIMEOUT_ENV: &str = "LOFT_REMOTE_TIMEOUT";
/// The variable that lets a run wait out a refusal the remote may lift.
pub const WAIT_ENV: &str = "LOFT_REMOTE_WAIT";

/// The form of the address a Loft server gives each document, as a literal that `concat!` takes.
macro_rules! document_address_form {
    () => {
        "https://<host>/loft/t/<tenant-slug>/api/v2/documents/<documentKey>/remote"
    };
}
pub(crate) use document_address_form;

/// The form of the address a Loft server gives the document of `key`.
#[must_use]
pub fn address_form_for(key: &str) -> String {
    document_address_form!().replace("<documentKey>", key)
}

/// How long a call to the remote may take, unless `LOFT_REMOTE_TIMEOUT` says otherwise.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Why the remote could not be reached, or what it refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteError {
    /// The document is connected but no address names its remote.
    NoAddress { key: String },
    /// The address has no sign-in, and `LOFT_REMOTE_TOKEN` is not set.
    NoSignIn { address: String },
    /// The sign-in kept under the address is for `resource`, a protected resource on another
    /// origin, which an earlier `loft` could keep, and `LOFT_REMOTE_TOKEN` is not set.
    SignInElsewhere { address: String, resource: String },
    /// The address is neither `https` nor `http` on the loopback interface.
    Address(String),
    /// The address holds `part`, a query, a fragment or user information, which no remote's
    /// address holds; `address` is the address without its user information.
    AddressPart { address: String, part: &'static str },
    /// `LOFT_REMOTE_TOKEN` is set and `LOFT_REMOTE_ENDPOINT` is not.
    TokenAlone,
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
    /// The remote refused, with `401`, a request for the document `key` that was sent without a
    /// token for want of a sign-in, `missing`.
    Unsigned {
        key: String,
        address: String,
        refusal: Refusal,
        missing: Box<RemoteError>,
    },
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
            Self::Address(address) => write!(
                formatter,
                "{address}: a remote's address is `https`, or `http` on the loopback interface"
            ),
            Self::AddressPart { address, part } => write!(
                formatter,
                "{address}: a remote's address holds no {part}: each request goes to the \
                 address followed by its endpoint's path"
            ),
            Self::TokenAlone => write!(
                formatter,
                "{TOKEN_ENV} is set and {ENDPOINT_ENV} is not: a token from the environment \
                 goes only to an address that {ENDPOINT_ENV} names, never to the one \
                 spec/loftspec.yaml names"
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
            Self::NoSignIn { .. }
            | Self::SignInElsewhere { .. }
            | Self::Lapsed { .. }
            | Self::Refused { .. }
            | Self::Unsigned { .. } => {
                let said = self
                    .said(Remedy::Own)
                    .expect("an error of a sign-in, or a refusal");
                formatter.write_str(&said)
            }
            Self::SignIn { address, reason } => {
                write!(formatter, "cannot sign in to {address}: {reason}")
            }
            Self::File(reason) => formatter.write_str(reason),
        }
    }
}

/// `address` as a remote's address, if it may be contacted and is an origin and a path alone:
/// each request goes to the address followed by its endpoint's path, which a query or a
/// fragment would swallow, and user information would travel with every request and be printed
/// with every error.
///
/// # Errors
///
/// The address is neither `https` nor `http` on the loopback interface, or holds a query, a
/// fragment or user information.
pub fn checked(address: &str) -> Result<&str, RemoteError> {
    let uri =
        network::allowed(address).ok_or_else(|| RemoteError::Address(without_user(address)))?;
    let user = (uri.authority()).is_some_and(|authority| authority.as_str().contains('@'));
    let part = if address.contains('#') {
        "fragment"
    } else if address.contains('?') {
        "query"
    } else if user {
        "user information"
    } else {
        return Ok(address);
    };
    Err(RemoteError::AddressPart {
        address: without_user(address),
        part,
    })
}

/// `address` as the address of a request, if it may be contacted, whatever else it holds: an
/// authorization server's endpoint may hold a query.
///
/// # Errors
///
/// The address is neither `https` nor `http` on the loopback interface.
pub fn contactable(address: &str) -> Result<&str, RemoteError> {
    network::allowed(address)
        .map(|_| address)
        .ok_or_else(|| RemoteError::Address(without_user(address)))
}

/// `address` without the user information before its host, so that no error prints a password
/// an address was given.
fn without_user(address: &str) -> String {
    let Some((scheme, rest)) = address.split_once("://") else {
        return address.to_owned();
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    format!("{scheme}://{host}{tail}")
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
/// A variable is not a whole number of seconds, there is no address, no sign-in or only one for a
/// protected resource on another origin, the address may not be contacted, or a refresh is
/// refused.
pub fn connected(remote: Option<&str>, key: &str) -> Result<Client, RemoteError> {
    reached(remote, key, false)
}

/// [`connected`], as `loft pull` reaches the remote: where the person has no sign-in to it that it
/// can send, none at all, one only for a protected resource on another origin or one whose refresh
/// is refused, the client sends no token, and the remote's `401` is answered with the error for
/// that sign-in.
///
/// # Errors
///
/// As [`connected`], but for a sign-in it cannot send.
pub fn reading(remote: Option<&str>, key: &str) -> Result<Client, RemoteError> {
    reached(remote, key, true)
}

/// The client of [`connected`], or, where `unsigned` allows and the person has no sign-in to the
/// remote, that of [`reading`].
fn reached(remote: Option<&str>, key: &str, unsigned: bool) -> Result<Client, RemoteError> {
    let timeout = timeout()?;
    let wait = seconds(WAIT_ENV)?;
    let endpoint = environment::text(ENDPOINT_ENV);
    let given = environment::text(TOKEN_ENV);
    // The manifest's address is the repository's to name, and a token from the environment is
    // not the repository's to send: it goes only where the environment says.
    if given.is_some() && endpoint.is_none() {
        return Err(RemoteError::TokenAlone);
    }
    let address = endpoint
        .or_else(|| remote.map(str::to_owned))
        .ok_or_else(|| RemoteError::NoAddress {
            key: key.to_owned(),
        })?;
    checked(&address)?;
    let token = match given {
        Some(token) => token,
        None => match signins::live(&address, timeout) {
            Ok(sign_in) => sign_in.access_token,
            Err(
                missing @ (RemoteError::NoSignIn { .. }
                | RemoteError::SignInElsewhere { .. }
                | RemoteError::Lapsed { .. }),
            ) if unsigned => {
                return Ok(Client::new(&address, None, timeout, wait).unsigned(key, missing));
            }
            Err(error) => return Err(error),
        },
    };
    Ok(Client::new(&address, Some(token), timeout, wait))
}

/// The access token of the person's sign-in to `address`, the remote of `whose`:
/// the one `loft/remotes.json` holds, refreshed where it needs to be; or else, where it holds none,
/// one only for a protected resource on another origin, or one whose refresh is refused, a new
/// sign-in kept there in its place, opened in the person's browser where `browser` allows.
///
/// # Errors
///
/// `loft/remotes.json` cannot be read or written, or the sign-in cannot be made.
pub fn signed_in(
    address: &str,
    whose: Whose<'_>,
    browser: bool,
    timeout: Duration,
) -> Result<String, RemoteError> {
    match signins::live(address, timeout) {
        Ok(sign_in) => Ok(sign_in.access_token),
        Err(
            RemoteError::NoSignIn { .. }
            | RemoteError::SignInElsewhere { .. }
            | RemoteError::Lapsed { .. },
        ) => {
            let sign_in = signin::sign_in(address, whose, browser, timeout)?;
            let token = sign_in.access_token.clone();
            signins::save(address, &sign_in)?;
            Ok(token)
        }
        Err(error) => Err(error),
    }
}

/// A new sign-in to `address`, the remote of `whose`, kept in `loft/remotes.json`, opened in the
/// person's browser where `browser` allows.
///
/// # Errors
///
/// The address may not be contacted, `LOFT_REMOTE_TIMEOUT` is not a whole number of seconds,
/// the sign-in cannot be made, or `loft/remotes.json` cannot be written.
pub fn login(address: &str, whose: Whose<'_>, browser: bool) -> Result<(), RemoteError> {
    checked(address)?;
    let sign_in = signin::sign_in(address, whose, browser, timeout()?)?;
    signins::save(address, &sign_in)
}

/// How a command that reaches another document's remote holds the person's sign-in to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignIn {
    /// A sign-in is made first where none is kept, in the person's browser where `browser`
    /// allows.
    First { browser: bool },
    /// Only a kept sign-in, refreshed where it needs to be, is used.
    Kept,
}

/// The client of another document's remote at `address`, the remote of `whose`, with the
/// person's sign-in to that address as `sign_in` holds it. For [`SignIn::First`], the client
/// signs the person in afresh, once, where the remote refuses that sign-in. Neither
/// `LOFT_REMOTE_ENDPOINT` nor `LOFT_REMOTE_TOKEN` is read: they name the document's own remote
/// alone.
///
/// # Errors
///
/// A timeout variable is not a whole number of seconds, the address may not be contacted, no
/// sign-in can be made, or, for [`SignIn::Kept`], none is kept or its refresh is refused.
pub fn other(address: &str, whose: Whose<'_>, sign_in: SignIn) -> Result<Client, RemoteError> {
    let timeout = timeout()?;
    let wait = seconds(WAIT_ENV)?;
    checked(address)?;
    let token = match sign_in {
        SignIn::First { browser } => {
            let token = signed_in(address, whose, browser, timeout)?;
            let client = Client::new(address, Some(token), timeout, wait);
            return Ok(client.renewing(whose, browser));
        }
        SignIn::Kept => signins::live(address, timeout)?.access_token,
    };
    Ok(Client::new(address, Some(token), timeout, wait))
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

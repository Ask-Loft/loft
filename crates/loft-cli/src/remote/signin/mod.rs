//! Signing in to a remote through OAuth 2.0: the authorization server is learnt from the
//! remote's challenge (RFC 9728, RFC 8414), `loft` registers itself as a public native client
//! (RFC 7591), and the authorization code grant with PKCE (RFC 7636) runs on a redirect to a
//! loopback port of its own (RFC 8252), naming the remote's protected resource (RFC 8707). A
//! refresh rotates the refresh token, and the caller keeps the successor before it uses it.

mod discovery;
mod listener;

use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use self::discovery::Discovered;
use self::listener::{Callback, Listener, Waited};
use super::signins::{SignIn, expires_at};
use super::wire::{base64url, query};
use super::{RemoteError, browser};
use crate::network;
use crate::out::errln;

/// How long a sign-in waits for the browser to come back, the lifetime of an authorization
/// request.
const CALLBACK_LIMIT: Duration = Duration::from_mins(5);

/// The redirect URI `loft` registers; each sign-in redirects to a port of its own on it.
const REGISTERED_REDIRECT: &str = "http://127.0.0.1/callback";

/// Signs the person in to the remote at `address`, opening the browser unless `open_browser` is
/// false, and gives the sign-in to keep.
///
/// # Errors
///
/// The remote's challenge or metadata cannot be followed, the authorization server refuses, or
/// the browser does not come back within five minutes.
pub fn sign_in(
    address: &str,
    open_browser: bool,
    timeout: Duration,
) -> Result<SignIn, RemoteError> {
    let failed = |reason: String| RemoteError::SignIn {
        address: address.to_owned(),
        reason,
    };
    let discovered = discovery::discover(address, timeout)?;
    let listener = Listener::bind().map_err(|error| failed(error.to_string()))?;
    let client_id = register(&discovered, timeout).map_err(failed)?;
    let verifier = random(32).map_err(failed)?;
    let state = random(16).map_err(failed)?;
    let challenge = base64url(&Sha256::digest(verifier.as_bytes()));
    let redirect_uri = listener.redirect_uri();
    let separator = if discovered.authorization_endpoint.contains('?') {
        '&'
    } else {
        '?'
    };
    let url = format!(
        "{}{separator}{}",
        discovered.authorization_endpoint,
        query(&[
            ("response_type", "code"),
            ("client_id", &client_id),
            ("redirect_uri", &redirect_uri),
            ("scope", &discovered.scope),
            ("resource", &discovered.resource),
            ("state", &state),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
        ])
    );
    errln!("To sign in to {address}, open this address in a browser:\n\n  {url}\n");
    let browser = if open_browser {
        let started = browser::open(&url);
        if started.is_none() {
            errln!("No browser could be opened: open the address above.");
        }
        started
    } else {
        None
    };
    let code = match listener.wait(&state, CALLBACK_LIMIT, browser) {
        Ok(Callback::Code(code)) => code,
        Ok(Callback::Error(error)) => {
            return Err(failed(format!(
                "the authorization server answered `{error}`"
            )));
        }
        Err(Waited::TimedOut) => {
            return Err(failed(
                "the browser did not return within 5 minutes".to_owned(),
            ));
        }
        Err(Waited::Failed(error)) => return Err(failed(error.to_string())),
    };
    let answer = token(
        &discovered.token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
            ("client_id", &client_id),
            ("code_verifier", &verifier),
            ("resource", &discovered.resource),
        ],
        timeout,
    )
    .map_err(|error| failed(error.to_string()))?;
    let issued =
        issued(&answer, None).ok_or_else(|| failed("the token answer does not read".to_owned()))?;
    Ok(SignIn {
        token_endpoint: discovered.token_endpoint,
        resource: discovered.resource,
        client_id,
        access_token: issued.0,
        expires_at: issued.1,
        refresh_token: issued.2,
    })
}

/// The sign-in `held` refreshed at its token endpoint.
///
/// # Errors
///
/// The authorization server refuses, which means the sign-in has lapsed, or cannot be reached.
pub fn refresh(address: &str, held: &SignIn, timeout: Duration) -> Result<SignIn, RemoteError> {
    let answer = token(
        &held.token_endpoint,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &held.refresh_token),
            ("client_id", &held.client_id),
            ("resource", &held.resource),
        ],
        timeout,
    )
    .map_err(|error| match error {
        TokenError::Refused(error) => RemoteError::Lapsed {
            address: address.to_owned(),
            error,
        },
        TokenError::Unreachable(reason) => RemoteError::Unreachable {
            address: held.token_endpoint.clone(),
            reason,
        },
    })?;
    let (access_token, expires_at, refresh_token) = issued(&answer, Some(&held.refresh_token))
        .ok_or_else(|| RemoteError::Lapsed {
            address: address.to_owned(),
            error: "the token answer does not read".to_owned(),
        })?;
    Ok(SignIn {
        access_token,
        expires_at,
        refresh_token,
        ..held.clone()
    })
}

/// `bytes` random bytes from the system's source, in base64url.
fn random(bytes: usize) -> Result<String, String> {
    let mut buffer = vec![0; bytes];
    getrandom::fill(&mut buffer).map_err(|error| error.to_string())?;
    Ok(base64url(&buffer))
}

/// Registers `loft` with the authorization server as a public native client, and gives its id.
fn register(discovered: &Discovered, timeout: Duration) -> Result<String, String> {
    let body = json!({
        "client_name": "loft",
        "redirect_uris": [REGISTERED_REDIRECT],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    let reply = network::call(
        "POST",
        &discovered.registration_endpoint,
        &[("Content-Type", "application/json")],
        Some(body.to_string().as_bytes()),
        timeout,
    )?;
    let answer = serde_json::from_slice::<Value>(&reply.body).unwrap_or(Value::Null);
    match answer["client_id"].as_str() {
        Some(client) if (200..300).contains(&reply.status) => Ok(client.to_owned()),
        _ => Err(format!(
            "the registration was refused with `{}`",
            answer["error"].as_str().unwrap_or("no error")
        )),
    }
}

/// Why the token endpoint gave no tokens.
#[derive(Debug)]
enum TokenError {
    Refused(String),
    Unreachable(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(error) => write!(formatter, "the token endpoint refused with `{error}`"),
            Self::Unreachable(reason) => {
                write!(formatter, "the token endpoint cannot be reached: {reason}")
            }
        }
    }
}

/// A form-encoded `POST` of `form` to `endpoint`, its JSON answer.
fn token(endpoint: &str, form: &[(&str, &str)], timeout: Duration) -> Result<Value, TokenError> {
    let reply = network::call(
        "POST",
        endpoint,
        &[("Content-Type", "application/x-www-form-urlencoded")],
        Some(query(form).as_bytes()),
        timeout,
    )
    .map_err(TokenError::Unreachable)?;
    let answer = serde_json::from_slice::<Value>(&reply.body).unwrap_or(Value::Null);
    if (200..300).contains(&reply.status) {
        Ok(answer)
    } else {
        Err(TokenError::Refused(
            answer["error"].as_str().unwrap_or("no error").to_owned(),
        ))
    }
}

/// The access token, its expiry and the refresh token of a token answer, keeping `previous` as
/// the refresh token where the answer names none.
fn issued(answer: &Value, previous: Option<&str>) -> Option<(String, String, String)> {
    if !answer["token_type"]
        .as_str()?
        .eq_ignore_ascii_case("bearer")
    {
        return None;
    }
    let refresh = answer["refresh_token"].as_str().or(previous)?;
    Some((
        answer["access_token"].as_str()?.to_owned(),
        expires_at(answer["expires_in"].as_u64()?),
        refresh.to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pkce_example_of_rfc_7636_gives_its_challenge() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            base64url(&Sha256::digest(verifier.as_bytes())),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn a_verifier_is_43_unreserved_characters_drawn_anew_each_time() {
        let (first, second) = (random(32).unwrap(), random(32).unwrap());
        assert_eq!(first.len(), 43);
        assert!(
            first
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        );
        assert_ne!(first, second);
    }
}

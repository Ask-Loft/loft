//! The remote's authorization server, as OAuth 2.0 shapes it: the protected resource metadata
//! (RFC 9728), the authorization server metadata (RFC 8414), dynamic registration of public
//! clients (RFC 7591), the authorization code grant with PKCE (RFC 7636) on a loopback redirect
//! whose port may differ from the registered one (RFC 8252), the resource named by RFC 8707, and
//! refresh tokens that rotate on every use, with a grace window for one presented again and the
//! whole sign-in revoked otherwise. Its clock runs from the machine's, moved on by the test.
//!
//! What the server holds and issues lies here; each endpoint lies in a module of its own.

mod authorize;
mod metadata;
mod register;
mod token;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::State;
use super::wire::{pairs, value};

pub(super) use authorize::authorize;
pub(super) use metadata::{document_metadata, resource_metadata, server_metadata};
pub(super) use register::register;
pub(super) use token::{refreshing, token};

/// A sign-in that the simulator issued: the client it was issued to and its tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// An authorization code, until it is exchanged.
struct Code {
    client_id: String,
    redirect_uri: String,
    challenge: String,
    resource: String,
    used: bool,
}

/// A refresh token: the sign-in it belongs to, and its rotation, once it has been rotated.
struct Refresh {
    family: usize,
    client_id: String,
    rotated: Option<Rotation>,
}

/// A refresh token's rotation: the answer it got, which issued its successor, and when.
struct Rotation {
    answer: Value,
    at: Instant,
}

/// One sign-in: its access tokens, each with the moment it expires, and whether it was
/// revoked.
#[derive(Default)]
struct Family {
    access: Vec<(String, Instant)>,
    revoked: bool,
}

/// What the authorization server holds.
#[derive(Default)]
pub(super) struct Auth {
    clients: HashMap<String, Vec<String>>,
    codes: HashMap<String, Code>,
    refresh: HashMap<String, Refresh>,
    families: Vec<Family>,
    issued: u64,
    pub(super) refreshes: usize,
    /// How far the test moved the clock on from the machine's.
    pub(super) moved: Duration,
}

impl Auth {
    /// The simulator's time: the machine's, moved on by the test.
    fn now(&self) -> Instant {
        Instant::now() + self.moved
    }

    fn next(&mut self, kind: &str) -> String {
        self.issued += 1;
        format!("{kind}-{}", self.issued)
    }

    /// Whether `token` is an access token, not yet expired, issued by a sign-in that was not
    /// revoked.
    pub(super) fn admits(&self, token: &str) -> bool {
        self.families.iter().any(|family| {
            !family.revoked
                && family
                    .access
                    .iter()
                    .any(|(issued, expires)| issued == token && self.now() < *expires)
        })
    }

    /// Begins a sign-in, and gives its family.
    fn sign_in(&mut self) -> usize {
        self.families.push(Family::default());
        self.families.len() - 1
    }

    /// Issues an access token and a refresh token in the sign-in `family`, as the answer of the
    /// token endpoint.
    fn issue(&mut self, family: usize, client_id: &str, expires_in: u64, scope: &str) -> Value {
        let access = self.next("access");
        let refresh = self.next("refresh");
        let expires = self.now() + Duration::from_secs(expires_in);
        self.families[family].access.push((access.clone(), expires));
        self.refresh.insert(
            refresh.clone(),
            Refresh {
                family,
                client_id: client_id.to_owned(),
                rotated: None,
            },
        );
        json!({
            "access_token": access,
            "token_type": "Bearer",
            "expires_in": expires_in,
            "refresh_token": refresh,
            "scope": scope,
        })
    }

    /// Whether a refresh token rotated as `rotation` is answered again: within `grace` of its
    /// rotation, and before its successor has itself been presented.
    fn repeats(&self, rotation: &Rotation, grace: Duration) -> bool {
        let successor_rotated = rotation.answer["refresh_token"]
            .as_str()
            .and_then(|successor| self.refresh.get(successor))
            .is_some_and(|successor| successor.rotated.is_some());
        self.now() < rotation.at + grace && !successor_rotated
    }

    /// A registered client and a new sign-in for it, without a browser.
    pub(super) fn grant(&mut self, expires_in: u64) -> super::Grant {
        let client_id = self.next("client");
        self.clients.insert(client_id.clone(), Vec::new());
        let family = self.sign_in();
        let answer = self.issue(family, &client_id, expires_in, "");
        super::Grant {
            client_id,
            access_token: answer["access_token"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            refresh_token: answer["refresh_token"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            expires_in,
        }
    }
}

impl State {
    pub(super) fn issuer(&self) -> String {
        format!("{}/auth", self.origin)
    }

    pub(super) fn resource(&self) -> String {
        format!("{}{}", self.origin, super::RESOURCE)
    }
}

/// A query string or a form-encoded body, read by field.
struct Form(Vec<(String, String)>);

impl Form {
    fn read(text: &str) -> Self {
        Self(pairs(text))
    }

    /// The value of the field `name`, or nothing.
    fn field(&self, name: &str) -> String {
        value(&self.0, name).unwrap_or_default().to_owned()
    }
}

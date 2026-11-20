//! The remote's authorization server, as OAuth 2.0 shapes it: the protected resource metadata
//! (RFC 9728), the authorization server metadata (RFC 8414), dynamic registration of public
//! clients (RFC 7591), the authorization code grant with PKCE (RFC 7636) on a loopback redirect
//! whose port may differ from the registered one (RFC 8252), one sign-in over each protected
//! resource its authorization request names (RFC 8707) with an access token for one resource
//! alone, and refresh tokens that rotate on every use, with a grace window for one presented again
//! for the resource its rotation named and the whole sign-in revoked otherwise. Its clock runs
//! from the machine's, moved on by the test.
//!
//! What the server holds and issues lies here; each endpoint lies in a module of its own.

mod authorize;
mod metadata;
mod register;
mod token;

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::State;
use super::wire::{pairs, value};

pub(super) use authorize::authorize;
pub(super) use metadata::{
    document_metadata, metadata_naming_elsewhere, resource_metadata, server_metadata,
};
pub(super) use register::register;
pub(super) use token::{refreshing, token};

/// A sign-in that the simulator issued: the client it was issued to and its tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub client_id: String,
    /// The access token issued for the first resource the sign-in holds.
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    /// The access token issued for each resource the sign-in holds, by the resource.
    pub access: BTreeMap<String, String>,
}

/// An authorization code, until it is exchanged: the resources and the scopes its authorization
/// request named.
struct Code {
    client_id: String,
    redirect_uri: String,
    challenge: String,
    resources: Vec<String>,
    scopes: Vec<String>,
    used: bool,
}

/// A refresh token: the sign-in it belongs to, and its rotation, once it has been rotated.
struct Refresh {
    family: usize,
    client_id: String,
    rotated: Option<Rotation>,
}

/// A refresh token's rotation: the resource it named, the answer it got, which issued its
/// successor, and when.
struct Rotation {
    resource: String,
    answer: Value,
    at: Instant,
}

/// One sign-in: the resources it holds, the scopes it was granted, its access tokens, and whether
/// it was revoked.
struct Family {
    resources: Vec<String>,
    scopes: Vec<String>,
    access: Vec<Access>,
    revoked: bool,
}

impl Family {
    /// Whether the sign-in holds `resource`.
    fn holds(&self, resource: &str) -> bool {
        self.resources.iter().any(|held| held == resource)
    }
}

/// An access token: the protected resource it was issued for alone, and the moment it expires.
struct Access {
    token: String,
    resource: String,
    expires: Instant,
}

/// An access token to issue: the protected resource it is for, the scopes that resource accepts,
/// and how many seconds it lasts.
struct Issued<'a> {
    resource: &'a str,
    accepted: &'a [String],
    expires_in: u64,
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

    /// Whether `token` is an access token issued for `resource`, not yet expired, by a sign-in
    /// that was not revoked.
    pub(super) fn admits_at(&self, resource: &str, token: &str) -> bool {
        self.families.iter().any(|family| {
            !family.revoked
                && family.access.iter().any(|access| {
                    access.token == token
                        && access.resource == resource
                        && self.now() < access.expires
                })
        })
    }

    /// Begins a sign-in that holds `resources`, granted `scopes`, and gives its family.
    fn sign_in(&mut self, resources: Vec<String>, scopes: Vec<String>) -> usize {
        self.families.push(Family {
            resources,
            scopes,
            access: Vec::new(),
            revoked: false,
        });
        self.families.len() - 1
    }

    /// Issues an access token in the sign-in `family` for `token.resource` alone, and gives it with
    /// its scope: the sign-in's scopes, in their order, that the resource accepts, separated by
    /// spaces.
    fn access(&mut self, family: usize, token: &Issued) -> (String, String) {
        let access = self.next("access");
        let expires = self.now() + Duration::from_secs(token.expires_in);
        let family = &mut self.families[family];
        family.access.push(Access {
            token: access.clone(),
            resource: token.resource.to_owned(),
            expires,
        });
        let scopes = family.scopes.iter();
        let narrowed: Vec<&str> = scopes
            .filter(|scope| token.accepted.contains(scope))
            .map(String::as_str)
            .collect();
        (access, narrowed.join(" "))
    }

    /// Issues a refresh token of the sign-in `family` to `client_id`.
    fn refresh_token(&mut self, family: usize, client_id: &str) -> String {
        let refresh = self.next("refresh");
        self.refresh.insert(
            refresh.clone(),
            Refresh {
                family,
                client_id: client_id.to_owned(),
                rotated: None,
            },
        );
        refresh
    }

    /// Issues an access token for `token.resource` and a refresh token in the sign-in `family`,
    /// as the answer of the token endpoint.
    fn issue(&mut self, family: usize, client_id: &str, token: &Issued) -> Value {
        let (access, scope) = self.access(family, token);
        let refresh = self.refresh_token(family, client_id);
        json!({
            "access_token": access,
            "token_type": "Bearer",
            "expires_in": token.expires_in,
            "refresh_token": refresh,
            "scope": scope,
            "resource": token.resource,
        })
    }

    /// Whether a refresh token rotated as `rotation`, presented again for `resource`, is answered
    /// again: for the resource its rotation named, within `grace` of its rotation, and before its
    /// successor has itself rotated.
    fn repeats(&self, rotation: &Rotation, resource: &str, grace: Duration) -> bool {
        let successor_rotated = rotation.answer["refresh_token"]
            .as_str()
            .and_then(|successor| self.refresh.get(successor))
            .is_some_and(|successor| successor.rotated.is_some());
        rotation.resource == resource && self.now() < rotation.at + grace && !successor_rotated
    }

    /// A registered client and a new sign-in for it, without a browser, over `resources`, one or
    /// more, each named once, as its caller ensures, each with the scopes it accepts: an access
    /// token for each resource alone, and one refresh token. The sign-in is granted every scope
    /// that one of its resources accepts.
    pub(super) fn grant(&mut self, expires_in: u64, resources: &[(String, Vec<String>)]) -> Grant {
        let client_id = self.next("client");
        self.clients.insert(client_id.clone(), Vec::new());
        let mut scopes: Vec<String> = Vec::new();
        for scope in resources.iter().flat_map(|(_, accepted)| accepted) {
            if !scopes.contains(scope) {
                scopes.push(scope.clone());
            }
        }
        let held = resources.iter().map(|(resource, _)| resource.clone());
        let family = self.sign_in(held.collect(), scopes);
        let mut access = BTreeMap::new();
        let mut first = None;
        for (resource, accepted) in resources {
            let token = Issued {
                resource,
                accepted,
                expires_in,
            };
            let (issued, _) = self.access(family, &token);
            first.get_or_insert_with(|| issued.clone());
            access.insert(resource.clone(), issued);
        }
        let refresh_token = self.refresh_token(family, &client_id);
        Grant {
            client_id,
            access_token: first.unwrap_or_default(),
            refresh_token,
            expires_in,
            access,
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

    /// Every value of the field `name`, in the order given.
    fn fields(&self, name: &str) -> Vec<String> {
        let named = self.0.iter().filter(|(field, _)| field == name);
        named.map(|(_, value)| value.clone()).collect()
    }
}

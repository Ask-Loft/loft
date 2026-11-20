//! What the contract suites of a remote's companion resources share, of the simulator and of a
//! real remote alike: the member of the remote's protected resource metadata that lists them, the
//! list read from metadata already fetched or found from a document's address, the origin an
//! address lies on, with its path and query, a document's head asked for with an access token,
//! and a sign-in as a client keeps it, made on the simulator or kept by `loft remote login` for a
//! real remote.

use std::{env, fs};

use loftspec::publish::PublishedAt;
use serde_json::{Value, json};
use software_plant::remote::Remote;
use ureq::http::Uri;

use super::calls::{Answer, call};
use super::oauth::{coded_for, exchange, fetch, refresh};
use super::signed_in::{kept_sign_in, real_now};

/// The member of a remote's protected resource metadata that lists its companion resources.
pub const COMPANION_RESOURCES: &str = "companion_resources";

/// The companion resources that the protected resource metadata of the document at `address`
/// lists, found from the challenge of a request without a token, as [`companions_of`] reads them.
pub fn companions_at(address: &str) -> Vec<(String, String)> {
    let (_, metadata, _) = fetch(address);
    companions_of(&metadata)
}

/// The companion resources that `metadata`, a remote's protected resource metadata, lists: each
/// as its `resource` and its `kind`, in the order listed, and none where it lists none.
///
/// # Panics
///
/// The member is not a list, or one of its entries holds no string `resource` or `kind`.
pub fn companions_of(metadata: &Value) -> Vec<(String, String)> {
    let Some(listed) = metadata.get(COMPANION_RESOURCES) else {
        return Vec::new();
    };
    let entries = listed
        .as_array()
        .unwrap_or_else(|| panic!("the companion resources are listed in an array: {listed}"));
    entries
        .iter()
        .map(|entry| {
            let field = |name: &str| {
                entry[name]
                    .as_str()
                    .unwrap_or_else(|| panic!("an entry with a string `{name}`: {entry}"))
                    .to_owned()
            };
            (field("resource"), field("kind"))
        })
        .collect()
}

/// The origin of the absolute URL `url`, as RFC 6454 compares origins, read by the parser that
/// `loft`'s requests use, as `loft` reads an origin: its scheme and its host in lower case, and
/// its port where it is not the scheme's default.
///
/// # Panics
///
/// `url` is not an absolute URL with a host.
pub fn origin_of(url: &str) -> String {
    origin_path_and_query(url).0
}

/// The absolute URL `url`, read by the parser that `loft`'s requests use, as `loft` reads an
/// address: its origin, as [`origin_of`] writes it; its path, `/` where it names none; and its
/// query with the `?` that opens it, or nothing where it has none. A fragment is never read.
///
/// # Panics
///
/// `url` is not an absolute URL with a host.
pub fn origin_path_and_query(url: &str) -> (String, String, String) {
    let uri: Uri = url
        .parse()
        .unwrap_or_else(|error| panic!("an absolute URL: {url}: {error}"));
    let (Some(scheme), Some(host)) = (uri.scheme_str(), uri.host()) else {
        panic!("an absolute URL with a host: {url}");
    };
    let scheme = scheme.to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    let default = match scheme.as_str() {
        "https" => Some(443),
        "http" => Some(80),
        _ => None,
    };
    let origin = match uri.port_u16().filter(|port| Some(*port) != default) {
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    };
    let query = uri
        .query()
        .map_or_else(String::new, |query| format!("?{query}"));
    (origin, uri.path().to_owned(), query)
}

/// The answer to `GET <address>/head` with the bearer token `token`, as a client signed in to the
/// document at `address` asks for its head.
pub fn head_with(address: &str, token: &str) -> Answer {
    call("GET", &format!("{address}/head"), Some(token), None)
}

/// A sign-in to the remote of the document at `address`, as a client keeps it: the token
/// endpoint, the client it was issued to, the protected resource it was made for, the access
/// token issued for that resource, and the refresh token.
pub struct Signed {
    pub address: String,
    pub token_endpoint: String,
    pub client: String,
    pub resource: String,
    pub access_token: String,
    pub refresh_token: String,
}

impl Signed {
    /// The authorization server's metadata, as far as a refresh reads it: its token endpoint.
    pub fn server(&self) -> Value {
        json!({ "token_endpoint": self.token_endpoint })
    }

    /// The answer to a refresh of `token` by the sign-in's client that names `resource`.
    pub fn refresh(&self, resource: &str, token: &str) -> Answer {
        refresh(&self.server(), &self.client, resource, token)
    }
}

/// A sign-in to `remote` made as a client makes one: a client registered, an authorization
/// request that names each of `resources` and asks for `scope`, which the simulator consents to,
/// and its code exchanged for the document's protected resource.
pub fn signed_over(remote: &Remote, resources: &[&str], scope: &str) -> Signed {
    let (server, client, code) = coded_for(remote, resources, scope);
    let resource = remote.resource();
    let answer = exchange(&server, &client, &resource, &code);
    assert_eq!(answer.status, 200, "{answer:?}");
    let field = |value: &Value, name: &str| value[name].as_str().unwrap().to_owned();
    Signed {
        address: remote.address(),
        token_endpoint: field(&server, "token_endpoint"),
        client,
        resource,
        access_token: field(&answer.body, "access_token"),
        refresh_token: field(&answer.body, "refresh_token"),
    }
}

/// The sign-in that `loft remote login` kept for the document at `LOFT_REMOTE_CONTRACT_ADDRESS`,
/// read from the `loft/remotes.json` that `LOFT_REMOTE_CONTRACT_SIGN_IN` names.
pub fn kept() -> Signed {
    let (address, sign_in) = kept_entry();
    signed_as_kept(address, &sign_in)
}

/// The sign-in that [`kept`] reads, whose access token must expire more than a minute from now
/// by the system clock: a remote refuses an expired access token whatever resource it was issued
/// for, so that a refusal of one would prove nothing of its resource.
pub fn kept_unexpired() -> Signed {
    let (address, sign_in) = kept_entry();
    let expires = sign_in["expiresAt"].as_str();
    let expires = expires.and_then(|at| at.parse::<PublishedAt>().ok());
    let expires =
        expires.unwrap_or_else(|| panic!("premise: a sign-in kept for {address} with `expiresAt`"));
    assert!(
        expires.unix_seconds() > real_now() + 60,
        "premise: the kept access token has not expired (it expires at {expires})"
    );
    signed_as_kept(address, &sign_in)
}

/// `LOFT_REMOTE_CONTRACT_ADDRESS`, and the entry for it of the `loft/remotes.json` that
/// `LOFT_REMOTE_CONTRACT_SIGN_IN` names.
fn kept_entry() -> (String, Value) {
    let address = env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let file = env::var("LOFT_REMOTE_CONTRACT_SIGN_IN").expect("a loft/remotes.json");
    let file: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    let sign_in = kept_sign_in(&file, &address).clone();
    (address, sign_in)
}

/// The sign-in to the document at `address` that `sign_in`, an entry of `loft/remotes.json`,
/// keeps.
fn signed_as_kept(address: String, sign_in: &Value) -> Signed {
    let field = |name: &str| {
        sign_in[name]
            .as_str()
            .unwrap_or_else(|| panic!("premise: a sign-in kept for {address} with `{name}`"))
            .to_owned()
    };
    Signed {
        token_endpoint: field("tokenEndpoint"),
        client: field("clientId"),
        resource: field("resource"),
        access_token: field("accessToken"),
        refresh_token: field("refreshToken"),
        address,
    }
}

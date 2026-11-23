//! Where a remote's authorization server is: the remote answers an unauthenticated request with
//! a `Bearer` challenge naming its protected resource metadata (RFC 9728), which names the
//! resource and the issuer, whose own metadata lies at its origin under
//! `/.well-known/oauth-authorization-server` followed by its path (RFC 8414). A sign-in follows
//! the challenge only to metadata on the origin of the remote's address, and takes only a
//! resource on that origin (the accepted decision "A remote's protected resource, and the
//! metadata its challenge names, lie on the origin of its address"), so that an address cannot
//! lead a sign-in to ask for another remote's resource and receive the access token issued for
//! it. It also reads the companion resources the remote lists, in `companions`.

use std::time::Duration;

use serde_json::Value;

use super::companions::{self, Companion};
use crate::network::{self, Reply};
use crate::remote::client::Client;
use crate::remote::wire::bearer_parameters;
use crate::remote::{RemoteError, Whose, contactable};

/// The path that protected resource metadata lies under (RFC 9728).
pub(super) const RESOURCE_METADATA: &str = "/.well-known/oauth-protected-resource";
/// The path that authorization server metadata lies under (RFC 8414).
const SERVER_METADATA: &str = "/.well-known/oauth-authorization-server";

/// What a sign-in learns of the remote's authorization server: the remote's protected resource,
/// the scope asked for, the issuer, the companion resources kept, and the endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    pub resource: String,
    pub scope: String,
    pub issuer: String,
    pub companions: Vec<Companion>,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: String,
}

/// The authorization server of the remote at `address`, the remote of `whose`.
///
/// # Errors
///
/// The remote gives no challenge, its challenge names metadata on another origin than
/// `address`, or a metadata document is missing, does not read, names a resource on another
/// origin than `address`, or is not one a sign-in can follow.
pub fn discover(
    address: &str,
    whose: Whose<'_>,
    timeout: Duration,
) -> Result<Discovered, RemoteError> {
    let failed = |reason: String| RemoteError::SignIn {
        address: address.to_owned(),
        reason,
    };
    let (asked, challenged) = challenge(address, timeout)?;
    let asked = format!("{address}/{asked}");
    let parameters = challenged
        .www_authenticate
        .iter()
        .filter(|_| challenged.status == 401)
        .find_map(|header| bearer_parameters(header))
        .ok_or_else(|| failed(unchallenged(&asked, whose, challenged.status)))?;
    let named = |name: &str| {
        parameters
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.clone())
    };
    let metadata_url = named("resource_metadata").ok_or_else(|| {
        failed(format!(
            "GET {asked} answered 401 with a `Bearer` challenge that names no \
             `resource_metadata`"
        ))
    })?;
    let (metadata, resource) =
        resource_metadata(address, &metadata_url, timeout).map_err(failed)?;
    let issuer = metadata["authorization_servers"][0]
        .as_str()
        .ok_or_else(|| {
            failed("the protected resource metadata names no authorization server".to_owned())
        })?;
    let server_url = server_metadata(issuer).ok_or_else(|| {
        failed(format!(
            "the protected resource metadata names an authorization server that is not an \
             address, {issuer}"
        ))
    })?;
    let server = fetch(&server_url, timeout).map_err(failed)?;
    if server["issuer"] != issuer {
        return Err(failed(format!(
            "the authorization server's metadata names another issuer than {issuer}"
        )));
    }
    let endpoint = |name: &str| -> Result<String, RemoteError> {
        let value = server[name]
            .as_str()
            .ok_or_else(|| failed(format!("the authorization server names no `{name}`")))?;
        contactable(value).map(str::to_owned)
    };
    let s256 = server["code_challenge_methods_supported"]
        .as_array()
        .is_some_and(|methods| methods.iter().any(|method| method == "S256"));
    if !s256 {
        return Err(failed(
            "the authorization server does not take PKCE with S256".to_owned(),
        ));
    }
    let authorization_endpoint = endpoint("authorization_endpoint")?;
    let token_endpoint = endpoint("token_endpoint")?;
    let registration_endpoint = endpoint("registration_endpoint")?;
    let companions = companions::kept(address, &resource, issuer, &metadata, timeout);
    let scope = companions::asked(&own_scope(named("scope"), &metadata), &companions);
    Ok(Discovered {
        resource,
        scope,
        issuer: issuer.to_owned(),
        companions,
        authorization_endpoint,
        token_endpoint,
        registration_endpoint,
    })
}

/// The remote's scope: `named`, the challenge's `scope`, or else every scope that `metadata`, the
/// remote's protected resource metadata, lists in `scopes_supported`, separated by spaces.
fn own_scope(named: Option<String>, metadata: &Value) -> String {
    named.unwrap_or_else(|| {
        metadata["scopes_supported"]
            .as_array()
            .map(|scopes| {
                scopes
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default()
    })
}

/// Where the metadata of the authorization server `issuer` lies, or none where the issuer is not
/// an absolute address: its origin, then the well-known path, then the issuer's own path without
/// its terminating `/` (RFC 8414, section 3.1). An issuer has no query or fragment (RFC 8414,
/// section 2), so the address carries none, and metadata found for an issuer that has one names
/// another issuer.
fn server_metadata(issuer: &str) -> Option<String> {
    let (origin, path) = network::origin_and_path(issuer)?;
    Some(format!(
        "{origin}{SERVER_METADATA}{}",
        path.trim_end_matches('/')
    ))
}

/// The protected resource metadata at `url`, which the challenge of the remote at `address`
/// names, and the protected resource it names, both on the address's origin (RFC 9728, sections
/// 7.3 and 7.4): metadata named on another origin is refused before anything is asked of it.
fn resource_metadata(
    address: &str,
    url: &str,
    timeout: Duration,
) -> Result<(Value, String), String> {
    if !network::same_origin(url, address) {
        return Err(format!(
            "the challenge names protected resource metadata on another origin than the \
             address, {url}"
        ));
    }
    let metadata = fetch(url, timeout)?;
    let resource = named_resource(address, url, &metadata)?;
    Ok((metadata, resource))
}

/// The `resource` that `metadata`, read at `url` for the remote at `address`, names: its own
/// address with the well-known path taken out (RFC 9728, section 3.3), on the address's origin.
fn named_resource(address: &str, url: &str, metadata: &Value) -> Result<String, String> {
    let resource = metadata["resource"].as_str().unwrap_or_default();
    if resource != url.replacen(RESOURCE_METADATA, "", 1) {
        return Err(format!(
            "the protected resource metadata at {url} names another resource, {resource}"
        ));
    }
    if !network::same_origin(resource, address) {
        return Err(format!(
            "the protected resource metadata at {url} names a resource on another origin than \
             the address, {resource}"
        ));
    }
    Ok(resource.to_owned())
}

/// The path below `address` of the unauthenticated request a sign-in reads the remote's challenge
/// from, and its answer: `head`, or, where the head answers `200`, as a remote answers for a
/// document it serves to anyone, the drafts of `main`, which it serves only to those it admits.
fn challenge(address: &str, timeout: Duration) -> Result<(&'static str, Reply), RemoteError> {
    let client = Client::new(address, None, timeout, None);
    let head = client.call("GET", "head", None)?;
    if head.status != 200 {
        return Ok(("head", head));
    }
    Ok((DRAFTS, client.call("GET", DRAFTS, None)?))
}

/// The path of the drafts of `main` below a document's address.
const DRAFTS: &str = "drafts/main";

/// Why an answer of `status` to the unauthenticated `GET <asked>` cannot start a sign-in: the
/// request and its status, and, for an answer a wrong address gives, that the address may not be
/// a document's remote, the form of the address of the document whose remote it is meant to be
/// and what mends it.
fn unchallenged(asked: &str, whose: Whose<'_>, status: u16) -> String {
    let refused = format!("GET {asked} answered {status}, not a 401 with a `Bearer` challenge");
    if matches!(status, 401 | 408 | 429) || status >= 500 {
        return refused;
    }
    format!(
        "{refused}, so the address may not be a document's remote. {}",
        whose.mend_address()
    )
}

/// The JSON object at `url`, an address a sign-in may contact.
pub(super) fn fetch(url: &str, timeout: Duration) -> Result<Value, String> {
    let reply = get(url, timeout).map_err(|error| format!("{url}: {error}"))?;
    document(url, &reply)
}

/// The answer to a `GET` of `url` that asks for JSON, or why none came in full within `timeout`.
pub(super) fn get(url: &str, timeout: Duration) -> Result<Reply, String> {
    super::asked("GET", url, &[("Accept", "application/json")], None, timeout)
}

/// The JSON object that `reply`, the answer to a `GET` of `url`, holds where it is answered
/// `200`; otherwise its status, named with `url`.
pub(super) fn document(url: &str, reply: &Reply) -> Result<Value, String> {
    serde_json::from_slice::<Value>(&reply.body)
        .ok()
        .filter(|value| reply.status == 200 && value.is_object())
        .ok_or_else(|| {
            format!(
                "{url} answered {} without a metadata document",
                reply.status
            )
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{named_resource, server_metadata};

    #[test]
    fn the_server_metadata_lies_at_the_issuer_s_origin_before_its_path() {
        let well_known = "/.well-known/oauth-authorization-server";
        for (issuer, origin, path) in [
            ("https://a.example/auth", "https://a.example", "/auth"),
            ("https://a.example", "https://a.example", ""),
            ("http://127.0.0.1:3000/", "http://127.0.0.1:3000", ""),
            (
                "https://a.example:8443/auth",
                "https://a.example:8443",
                "/auth",
            ),
            // The path's terminating slash is left out.
            (
                "https://a.example/tenants/one/",
                "https://a.example",
                "/tenants/one",
            ),
            // An issuer has no query (RFC 8414, section 2), and the address carries none: the
            // metadata found there names another issuer, which is refused.
            ("https://a.example?tenant=one", "https://a.example", ""),
            (
                "https://a.example/auth?tenant=one",
                "https://a.example",
                "/auth",
            ),
        ] {
            assert_eq!(
                server_metadata(issuer),
                Some(format!("{origin}{well_known}{path}")),
                "{issuer}"
            );
        }
        for issuer in ["as.example/auth", "://as.example/auth", "https://:443/auth"] {
            assert_eq!(server_metadata(issuer), None, "{issuer}");
        }
    }

    #[test]
    fn metadata_naming_a_resource_on_another_origin_than_the_address_is_refused() {
        let address = "https://h.example/documents/one";
        let well_known = "https://h.example/.well-known/oauth-protected-resource";
        let named = |suffix: &str, resource: &str| {
            let metadata = json!({ "resource": resource });
            named_resource(address, &format!("{well_known}{suffix}"), &metadata)
        };
        assert_eq!(
            named("/documents", "https://h.example/documents"),
            Ok("https://h.example/documents".to_owned())
        );
        let misnamed = named("/documents", "https://h.example/elsewhere").unwrap_err();
        assert!(misnamed.contains("names another resource"), "{misnamed}");
        // Each path lies on the address's origin, and its own address names a resource off it.
        for (suffix, resource) in [
            (
                "@g.example/documents",
                "https://h.example@g.example/documents",
            ),
            (".net/documents", "https://h.example.net/documents"),
            (":8443/documents", "https://h.example:8443/documents"),
        ] {
            let refused = named(suffix, resource).unwrap_err();
            assert!(
                refused.contains("on another origin") && refused.contains(resource),
                "{suffix}: {refused}"
            );
        }
    }
}

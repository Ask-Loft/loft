//! Where a remote's authorization server is: the remote answers an unauthenticated request with
//! a `Bearer` challenge naming its protected resource metadata (RFC 9728), which names the
//! resource and the issuer, whose own metadata lies at its origin under
//! `/.well-known/oauth-authorization-server` followed by its path (RFC 8414).

use std::time::Duration;

use serde_json::Value;

use crate::network;
use crate::remote::client::Client;
use crate::remote::wire::bearer_parameters;
use crate::remote::{RemoteError, checked};

/// The path that protected resource metadata lies under (RFC 9728).
const RESOURCE_METADATA: &str = "/.well-known/oauth-protected-resource";
/// The path that authorization server metadata lies under (RFC 8414).
const SERVER_METADATA: &str = "/.well-known/oauth-authorization-server";

/// What a sign-in learns of the remote's authorization server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    pub resource: String,
    pub scope: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: String,
}

/// The authorization server of the remote at `address`.
///
/// # Errors
///
/// The remote gives no challenge, or a metadata document is missing, does not read, or is not
/// one a sign-in can follow.
pub fn discover(address: &str, timeout: Duration) -> Result<Discovered, RemoteError> {
    let failed = |reason: String| RemoteError::SignIn {
        address: address.to_owned(),
        reason,
    };
    checked(address)?;
    let challenged = Client::new(address, None, timeout, None).call("GET", "head", None)?;
    let parameters = challenged
        .www_authenticate
        .as_deref()
        .filter(|_| challenged.status == 401)
        .and_then(bearer_parameters)
        .ok_or_else(|| {
            failed(format!(
                "the remote answered {} without a `Bearer` challenge",
                challenged.status
            ))
        })?;
    let named = |name: &str| {
        parameters
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.clone())
    };
    let metadata_url = named("resource_metadata")
        .ok_or_else(|| failed("the challenge names no `resource_metadata`".to_owned()))?;
    let metadata = fetch(&metadata_url, timeout).map_err(failed)?;
    let resource = metadata["resource"].as_str().unwrap_or_default().to_owned();
    if resource != metadata_url.replacen(RESOURCE_METADATA, "", 1) {
        return Err(failed(format!(
            "the protected resource metadata at {metadata_url} names another resource, {resource}"
        )));
    }
    let issuer = metadata["authorization_servers"][0]
        .as_str()
        .ok_or_else(|| {
            failed("the protected resource metadata names no authorization server".to_owned())
        })?;
    let server = fetch(&server_metadata(issuer), timeout).map_err(failed)?;
    if server["issuer"] != issuer {
        return Err(failed(format!(
            "the authorization server's metadata names another issuer than {issuer}"
        )));
    }
    let endpoint = |name: &str| -> Result<String, RemoteError> {
        let value = server[name]
            .as_str()
            .ok_or_else(|| failed(format!("the authorization server names no `{name}`")))?;
        checked(value).map(str::to_owned)
    };
    let s256 = server["code_challenge_methods_supported"]
        .as_array()
        .is_some_and(|methods| methods.iter().any(|method| method == "S256"));
    if !s256 {
        return Err(failed(
            "the authorization server does not take PKCE with S256".to_owned(),
        ));
    }
    let scope = named("scope").unwrap_or_else(|| {
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
    });
    Ok(Discovered {
        resource,
        scope,
        authorization_endpoint: endpoint("authorization_endpoint")?,
        token_endpoint: endpoint("token_endpoint")?,
        registration_endpoint: endpoint("registration_endpoint")?,
    })
}

/// Where the metadata of the authorization server `issuer` lies: its origin, then the
/// well-known path, then the issuer's own path.
fn server_metadata(issuer: &str) -> String {
    let authority = issuer.find("://").map_or(0, |at| at + 3);
    let end = issuer[authority..]
        .find('/')
        .map_or(issuer.len(), |at| authority + at);
    let path = issuer[end..].trim_end_matches('/');
    format!("{}{SERVER_METADATA}{path}", &issuer[..end])
}

/// The JSON object at `url`, an address a sign-in may contact.
fn fetch(url: &str, timeout: Duration) -> Result<Value, String> {
    let reply = network::call("GET", url, &[("Accept", "application/json")], None, timeout)
        .map_err(|error| format!("{url}: {error}"))?;
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
    use super::server_metadata;

    #[test]
    fn the_server_metadata_lies_at_the_issuer_s_origin_before_its_path() {
        assert_eq!(
            server_metadata("https://a.example/auth"),
            "https://a.example/.well-known/oauth-authorization-server/auth"
        );
        assert_eq!(
            server_metadata("https://a.example"),
            "https://a.example/.well-known/oauth-authorization-server"
        );
        assert_eq!(
            server_metadata("http://127.0.0.1:3000/"),
            "http://127.0.0.1:3000/.well-known/oauth-authorization-server"
        );
    }
}

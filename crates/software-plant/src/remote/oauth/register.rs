//! Dynamic registration of public clients (RFC 7591): a client names its redirect URIs, each
//! `https` or `http` on the loopback interface, and the grants it uses, and authenticates at the
//! token endpoint with no secret. A loopback redirect matches a registered one on any port
//! (RFC 8252).

use serde_json::{Value, json};

use crate::http::{Request, Response};
use crate::remote::State;
use crate::remote::problem::oauth;

/// Whether `uri` is `http` on the loopback interface.
pub(super) fn loopback(uri: &str) -> bool {
    uri.strip_prefix("http://").is_some_and(|rest| {
        let host = rest.split(['/', '?']).next().unwrap_or_default();
        let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
        matches!(host, "127.0.0.1" | "[::1]" | "localhost")
    })
}

/// A URI without the port of its authority, for matching a loopback redirect.
pub(super) fn portless(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("http://") else {
        return uri.to_owned();
    };
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let host = authority
        .rsplit_once(':')
        .map_or(authority, |(host, _)| host);
    format!("http://{host}{path}")
}

pub(crate) fn register(state: &mut State, request: &Request) -> Response {
    if let Some(error) = &state.scenario.refused_registration {
        return oauth(error);
    }
    let Ok(metadata) = serde_json::from_slice::<Value>(&request.body) else {
        return oauth("invalid_client_metadata");
    };
    let uris: Vec<String> = metadata["redirect_uris"]
        .as_array()
        .map(|uris| {
            uris.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if uris.is_empty()
        || uris
            .iter()
            .any(|uri| !uri.starts_with("https://") && !loopback(uri))
    {
        return oauth("invalid_redirect_uri");
    }
    let mut grants: Vec<&str> = metadata["grant_types"]
        .as_array()
        .map(|grants| grants.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    grants.sort_unstable();
    if metadata["token_endpoint_auth_method"] != "none"
        || grants != ["authorization_code", "refresh_token"]
    {
        return oauth("invalid_client_metadata");
    }
    let client_id = state.auth.next("client");
    state.auth.clients.insert(client_id.clone(), uris.clone());
    let mut answer = metadata;
    answer["client_id"] = json!(client_id);
    answer["client_id_issued_at"] = json!(state.scenario.publish_from);
    Response::json(201, &answer)
}

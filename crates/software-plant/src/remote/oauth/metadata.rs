//! The two metadata documents a client finds the authorization server by: the protected
//! resource metadata (RFC 9728), which names the resource and the issuer, and the authorization
//! server metadata (RFC 8414), which names the issuer's endpoints and what they take, each as the
//! contract states it or as a misconfigured scenario serves it.

use serde_json::json;

use crate::http::Response;
use crate::remote::{Metadata, State};

pub(crate) fn resource_metadata(state: &State) -> Response {
    let resource = match state.scenario.metadata {
        Metadata::MisnamedResource => format!("{}/elsewhere", state.origin),
        _ => state.resource(),
    };
    let scopes = match &state.scenario.metadata {
        Metadata::Unscoped(scopes) => json!(scopes),
        _ => json!([state.scenario.scope]),
    };
    Response::json(
        200,
        &json!({
            "resource": resource,
            "authorization_servers": [state.issuer()],
            "scopes_supported": scopes,
            "bearer_methods_supported": ["header"],
        }),
    )
}

pub(crate) fn server_metadata(state: &State) -> Response {
    let issuer = state.issuer();
    let mut metadata = json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "registration_endpoint": format!("{issuer}/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": [state.scenario.scope],
    });
    match state.scenario.metadata {
        Metadata::MisnamedIssuer => metadata["issuer"] = json!(format!("{issuer}/elsewhere")),
        Metadata::Unregistering => {
            if let Some(fields) = metadata.as_object_mut() {
                fields.remove("registration_endpoint");
            }
        }
        Metadata::Plain => metadata["code_challenge_methods_supported"] = json!(["plain"]),
        Metadata::Unencrypted => {
            metadata["authorization_endpoint"] = json!("http://auth.example/authorize");
        }
        _ => {}
    }
    Response::json(200, &metadata)
}

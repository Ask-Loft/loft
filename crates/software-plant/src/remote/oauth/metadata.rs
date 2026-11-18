//! The two metadata documents a client finds the authorization server by: the protected
//! resource metadata (RFC 9728), which names the resource, the issuer and the related resources,
//! and the authorization server metadata (RFC 8414), which names the issuer's endpoints and what
//! they take, each as the contract states it or as a misconfigured scenario serves it.

use serde_json::{Value, json};

use crate::http::Response;
use crate::remote::{Metadata, RESOURCE, State};

pub(crate) fn resource_metadata(state: &State) -> Response {
    Response::json(200, &document_metadata(state, &state.origin))
}

/// The protected resource metadata of the document's protected resource, as `origin` serves it.
pub(crate) fn document_metadata(state: &State, origin: &str) -> Value {
    let resource = match state.scenario.metadata {
        Metadata::MisnamedResource => format!("{origin}/elsewhere"),
        _ => format!("{origin}{RESOURCE}"),
    };
    let scopes = match &state.scenario.metadata {
        Metadata::Unscoped(scopes) => json!(scopes),
        _ => json!([state.scenario.scope]),
    };
    let mut metadata = json!({
        "resource": resource,
        "authorization_servers": [state.issuer()],
        "scopes_supported": scopes,
        "bearer_methods_supported": ["header"],
    });
    state.list_related(&mut metadata);
    metadata
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

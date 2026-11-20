//! The two metadata documents a client finds the authorization server by: the protected
//! resource metadata (RFC 9728), which names the resource, the issuer and the companion resources,
//! and the authorization server metadata (RFC 8414), which names the issuer's endpoints and what
//! they take, each as the contract states it or as a misconfigured scenario serves it.

use serde_json::{Value, json};

use crate::http::Response;
use crate::remote::{Metadata, RESOURCE, State, WELL_KNOWN};

pub(crate) fn resource_metadata(state: &State) -> Response {
    Response::json(200, &document_metadata(state, &state.origin))
}

/// The document's protected resource metadata as a scenario that names its protected resource on
/// another origin serves it at `path` on the simulator's origin: naming as `resource` the
/// metadata's address with the well-known path taken out, as RFC 9728 derives a resource from it,
/// which lies on the second port's origin.
pub(crate) fn metadata_naming_elsewhere(state: &State, path: &str) -> Response {
    let mut metadata = document_metadata(state, &state.origin);
    let address = format!("{}{path}", state.origin);
    metadata["resource"] = json!(address.replacen(WELL_KNOWN, "", 1));
    Response::json(200, &metadata)
}

/// The protected resource metadata of the document's protected resource, as `origin` serves it.
pub(crate) fn document_metadata(state: &State, origin: &str) -> Value {
    let resource = match state.scenario.metadata {
        Metadata::MisnamedResource => format!("{origin}/elsewhere"),
        _ => format!("{origin}{RESOURCE}"),
    };
    let mut metadata = json!({
        "resource": resource,
        "authorization_servers": [state.issuer()],
        "scopes_supported": state.document_scopes(),
        "bearer_methods_supported": ["header"],
    });
    state.list_companions(&mut metadata);
    metadata
}

impl State {
    /// The scopes that the document's protected resource metadata lists in `scopes_supported`:
    /// those a scenario that names no challenged scope names, or else the challenged scope.
    pub(crate) fn document_scopes(&self) -> Vec<String> {
        match &self.scenario.metadata {
            Metadata::Unscoped(scopes) => scopes.clone(),
            _ => vec![self.scenario.scope.clone()],
        }
    }
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

//! The protected resources beside the document's that one sign-in to the remote covers, which
//! the document's protected resource metadata lists by identifier and kind: the metadata each one
//! serves where RFC 9728 places it, faithfully or with the one fault its scenario names, which of
//! them the authorization server serves and the scopes each accepts, and the second port of the
//! loopback interface that stands for another origin. A related resource has no routes of its
//! own.

use serde_json::{Value, json};

use super::oauth::document_metadata;
use super::problem::refusal;
use super::{Metadata, RESOURCE, RemoteScenario, State, only};
use crate::http::{Request, Response};

/// The member of a protected resource metadata document that lists the related resources.
const RELATED_RESOURCES: &str = "related_resources";

/// Where a protected resource metadata document lies on its resource's origin, before the
/// resource's path and query (RFC 9728).
const WELL_KNOWN: &str = "/.well-known/oauth-protected-resource";

/// A protected resource that the document's protected resource metadata lists as related: its
/// kind, its path on its origin, the scopes its own metadata lists, and the fault its scenario
/// serves it with, if any.
#[derive(Clone, Debug)]
pub struct Related {
    kind: String,
    path: String,
    scopes: Vec<String>,
    fault: Option<RelatedFault>,
}

impl Related {
    /// A related resource of `kind` at `<origin>/<kind>`, whose protected resource metadata lists
    /// `scopes` and is served faithfully.
    #[must_use]
    pub fn new(kind: &str, scopes: &[&str]) -> Self {
        Self {
            kind: kind.to_owned(),
            path: format!("/{kind}"),
            scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
            fault: None,
        }
    }

    /// Places the resource at `path` on its origin: a path that begins with `/`, and may end in
    /// a query.
    #[must_use]
    pub fn at(mut self, path: &str) -> Self {
        path.clone_into(&mut self.path);
        self
    }

    /// Serves the resource with `fault`, in place of any fault named before.
    #[must_use]
    pub fn faulty(mut self, fault: RelatedFault) -> Self {
        self.fault = Some(fault);
        self
    }

    /// Whether the resource lies on the second port, which stands for another origin.
    fn elsewhere(&self) -> bool {
        self.fault == Some(RelatedFault::OnAnotherOrigin)
    }
}

/// How a scenario misserves a related resource, or where else it places one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelatedFault {
    /// Listed, and its metadata served faithfully, on another origin: a second port of the
    /// loopback interface, which records its requests apart.
    OnAnotherOrigin,
    /// Metadata whose `authorization_servers` names another issuer than the document's.
    UnderAnotherIssuer,
    /// Metadata whose `resource` is not the identifier listed for it.
    Misnamed,
    /// Metadata without `resource`.
    Unnamed,
    /// Metadata without `scopes_supported`, as metadata that lists no scope omits it.
    Scopeless,
    /// Metadata whose `scopes_supported` holds no scope.
    EmptyScopes,
    /// Metadata that is not found: its address is answered `404 Not Found`.
    Unreadable,
    /// Metadata that is a JSON array, not an object.
    NotAnObject,
    /// Faithful metadata that lists a related resource of its own.
    Relisting,
}

impl RemoteScenario {
    /// Lists `related` after the related resources listed before it, and serves its metadata.
    #[must_use]
    pub fn relating(mut self, related: Related) -> Self {
        self.related.push(related);
        self
    }

    /// Answers the document's protected resource metadata with `raw` as the member that lists
    /// the related resources, in place of the list they make, each `{origin}` in a string of it
    /// read as the simulator's origin. The related resources keep their own metadata.
    #[must_use]
    pub fn listing(mut self, raw: Value) -> Self {
        self.listing = Some(raw);
        self
    }

    /// Whether the scenario places metadata on another origin, which a second port stands for.
    pub(super) fn places_elsewhere(&self) -> bool {
        self.metadata == Metadata::OnAnotherOrigin || self.related.iter().any(Related::elsewhere)
    }
}

impl State {
    /// The identifier of `related`: its path on the simulator's origin, or on the second port's
    /// for a resource placed there.
    fn identifier(&self, related: &Related) -> String {
        let origin = match &self.elsewhere {
            Some(elsewhere) if related.elsewhere() => elsewhere,
            _ => &self.origin,
        };
        format!("{origin}{}", related.path)
    }

    /// The identifier that the document's protected resource metadata lists for the first
    /// related resource of `kind`.
    pub(super) fn related_identifier(&self, kind: &str) -> Option<String> {
        self.scenario
            .related
            .iter()
            .find(|related| related.kind == kind)
            .map(|related| self.identifier(related))
    }

    /// The origin on which the challenge names the document's protected resource metadata: the
    /// second port's for a scenario that places it there, the simulator's otherwise.
    pub(super) fn metadata_origin(&self) -> &str {
        match &self.elsewhere {
            Some(elsewhere) if self.scenario.metadata == Metadata::OnAnotherOrigin => elsewhere,
            _ => &self.origin,
        }
    }

    /// Adds to `metadata`, the document's protected resource metadata, the member that lists the
    /// related resources: the value its scenario scripts, or else an object of `resource` and
    /// `kind` for each related resource, in the order listed. A scenario with neither adds none.
    pub(super) fn list_related(&self, metadata: &mut Value) {
        if let Some(raw) = &self.scenario.listing {
            metadata[RELATED_RESOURCES] = with_origin(raw, &self.origin);
        } else if !self.scenario.related.is_empty() {
            let listed = self.scenario.related.iter().map(
                |related| json!({ "resource": self.identifier(related), "kind": related.kind }),
            );
            metadata[RELATED_RESOURCES] = Value::Array(listed.collect());
        }
    }

    /// The answer on the simulator's origin to `GET` of `target`, a path and its query, where the
    /// protected resource metadata of a related resource there lies.
    pub(super) fn related_metadata(&self, target: &str) -> Option<Response> {
        self.placed(target, false)
            .and_then(|related| self.served(related))
            .map(|metadata| Response::json(200, &metadata))
    }

    /// The scopes that `resource` accepts, if the scenario serves it: the document's protected
    /// resource, or a related resource on the simulator's origin whose metadata names it, lists
    /// the simulator's issuer and lists a scope. They are the scopes its metadata lists in
    /// `scopes_supported`.
    pub(super) fn accepted(&self, resource: &str) -> Option<Vec<String>> {
        if resource == self.resource() {
            return Some(self.document_scopes());
        }
        let related = self.scenario.related.iter();
        related
            .filter(|related| !related.elsewhere() && self.identifier(related) == resource)
            .find_map(|related| self.vouched(related))
    }

    /// The scopes that the metadata of `related`, as its scenario serves it, lists, where that
    /// metadata names the resource and the simulator's issuer and lists a scope.
    fn vouched(&self, related: &Related) -> Option<Vec<String>> {
        let metadata = self.served(related)?;
        let issuer = json!(self.issuer());
        let names_itself = metadata["resource"] == json!(self.identifier(related));
        let servers = metadata["authorization_servers"].as_array();
        let under_the_issuer = servers.is_some_and(|servers| servers.contains(&issuer));
        let listed = metadata["scopes_supported"]
            .as_array()
            .into_iter()
            .flatten();
        let scopes: Vec<String> = listed
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        (names_itself && under_the_issuer && !scopes.is_empty()).then_some(scopes)
    }

    /// The related resource whose protected resource metadata lies at `target` on the second
    /// port, if `elsewhere`, or on the simulator's origin otherwise.
    fn placed(&self, target: &str, elsewhere: bool) -> Option<&Related> {
        self.scenario
            .related
            .iter()
            .find(|related| related.elsewhere() == elsewhere && target == well_known(&related.path))
    }

    /// The protected resource metadata of `related` as its scenario serves it, or nothing for
    /// metadata it makes unreadable.
    fn served(&self, related: &Related) -> Option<Value> {
        let identifier = self.identifier(related);
        let issuer = self.issuer();
        let mut metadata = json!({
            "resource": identifier,
            "authorization_servers": [issuer],
            "scopes_supported": related.scopes,
            "bearer_methods_supported": ["header"],
        });
        match related.fault {
            None | Some(RelatedFault::OnAnotherOrigin) => {}
            Some(RelatedFault::UnderAnotherIssuer) => {
                metadata["authorization_servers"] = json!([format!("{issuer}/elsewhere")]);
            }
            Some(RelatedFault::Misnamed) => {
                metadata["resource"] = json!(format!("{identifier}/elsewhere"));
            }
            Some(RelatedFault::Unnamed) => metadata = without(metadata, "resource"),
            Some(RelatedFault::Scopeless) => metadata = without(metadata, "scopes_supported"),
            Some(RelatedFault::EmptyScopes) => metadata["scopes_supported"] = json!([]),
            Some(RelatedFault::Unreadable) => return None,
            Some(RelatedFault::NotAnObject) => metadata = json!([metadata]),
            Some(RelatedFault::Relisting) => {
                let relisted = format!("{}/relisted", self.origin);
                metadata[RELATED_RESOURCES] = json!([{ "resource": relisted, "kind": "relisted" }]);
            }
        }
        Some(metadata)
    }
}

/// The answer of the second port, which stands for another origin: the metadata of each related
/// resource placed there, faithfully, and of the document's protected resource for a scenario
/// that places it there; `not_found` to every other request.
pub(super) fn elsewhere(state: &mut State, request: &Request) -> Response {
    let moved = state.scenario.metadata == Metadata::OnAnotherOrigin
        && request.path == well_known(RESOURCE);
    let metadata = if moved {
        Some(document_metadata(state, state.metadata_origin()))
    } else {
        state
            .placed(&request.path, true)
            .and_then(|related| state.served(related))
    };
    match metadata {
        Some(metadata) => only("GET", request, || Response::json(200, &metadata)),
        None => refusal("not_found"),
    }
}

/// Where RFC 9728 places the protected resource metadata of the resource at `path`, a path that
/// may end in a query: `/.well-known/oauth-protected-resource`, then the path without a
/// terminating `/`, then the query.
fn well_known(path: &str) -> String {
    let (path, query) = path.find('?').map_or((path, ""), |at| path.split_at(at));
    format!("{WELL_KNOWN}{}{query}", path.trim_end_matches('/'))
}

/// `metadata` without its member `name`.
fn without(mut metadata: Value, name: &str) -> Value {
    if let Value::Object(fields) = &mut metadata {
        fields.remove(name);
    }
    metadata
}

/// `value` with each `{origin}` in a string it holds read as `origin`.
fn with_origin(value: &Value, origin: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace("{origin}", origin)),
        Value::Array(items) => items.iter().map(|item| with_origin(item, origin)).collect(),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(name, item)| (name.clone(), with_origin(item, origin)))
                .collect(),
        ),
        other => other.clone(),
    }
}

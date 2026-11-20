//! The protected resources beside the document's that one sign-in to the remote covers, which
//! the document's protected resource metadata lists by identifier and kind: where each one lies,
//! on the simulator's origin or on the second port of the loopback interface that stands for
//! another origin; the metadata each one serves there, where RFC 9728 places it, faithfully or
//! with the one fault its scenario names; and which of them the authorization server serves, with
//! the scopes each accepts. A companion resource has no routes of its own.

use serde_json::{Value, json};

use super::oauth::document_metadata;
use super::problem::refusal;
use super::{Metadata, RESOURCE, RemoteScenario, State, WELL_KNOWN, only};
use crate::http::{Request, Response};

/// The member of a protected resource metadata document that lists the companion resources.
const COMPANION_RESOURCES: &str = "companion_resources";

/// A protected resource that the document's protected resource metadata lists as a companion:
/// its kind, its path on its origin, whether that origin is another than the simulator's, the
/// scopes its own metadata lists, and the fault its scenario serves that metadata with, if any.
#[derive(Clone, Debug)]
pub struct Companion {
    kind: String,
    path: String,
    /// Whether the resource lies on the second port, which stands for another origin.
    elsewhere: bool,
    scopes: Vec<String>,
    fault: Option<CompanionFault>,
}

impl Companion {
    /// A companion resource of `kind` at `/<kind>` on the simulator's origin, whose protected
    /// resource metadata lists `scopes` and is served faithfully.
    #[must_use]
    pub fn new(kind: &str, scopes: &[&str]) -> Self {
        Self {
            kind: kind.to_owned(),
            path: format!("/{kind}"),
            elsewhere: false,
            scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
            fault: None,
        }
    }

    /// Places the resource at `path` on its origin: a path that begins with `/`, and may end in
    /// a query.
    ///
    /// # Panics
    ///
    /// `path` does not begin with `/`.
    #[must_use]
    pub fn at(mut self, path: &str) -> Self {
        assert!(
            path.starts_with('/'),
            "a companion resource's path begins with `/`: {path:?}"
        );
        path.clone_into(&mut self.path);
        self
    }

    /// Places the resource on another origin: a second port of the loopback interface, which
    /// records its requests apart. Its metadata is served there as its scenario serves it,
    /// faithfully or with its fault; and the remote does not serve the resource, so that an
    /// authorization request or a token request that names it is refused `invalid_target`.
    #[must_use]
    pub fn elsewhere(mut self) -> Self {
        self.elsewhere = true;
        self
    }

    /// Serves the resource's metadata with `fault`, in place of any fault named before.
    #[must_use]
    pub fn faulty(mut self, fault: CompanionFault) -> Self {
        self.fault = Some(fault);
        self
    }

    /// Where the resource's protected resource metadata lies: whether on the second port, and at
    /// which path and query there.
    fn metadata_at(&self) -> (bool, String) {
        (self.elsewhere, well_known(&self.path))
    }

    /// The scopes that the resource accepts, if the metadata its scenario serves vouches for it:
    /// names it and the simulator's issuer, and lists a scope, as metadata served faithfully, or
    /// with a companion resource of its own, does where the resource lists a scope. Every other
    /// fault leaves the resource unserved; so does another origin, which its caller judges.
    fn vouched(&self) -> Option<Vec<String>> {
        match self.fault {
            None | Some(CompanionFault::Relisting) => {
                (!self.scopes.is_empty()).then(|| self.scopes.clone())
            }
            Some(
                CompanionFault::UnderAnotherIssuer
                | CompanionFault::Misnamed
                | CompanionFault::Unnamed
                | CompanionFault::Scopeless
                | CompanionFault::EmptyScopes
                | CompanionFault::Unreadable
                | CompanionFault::NotAnObject,
            ) => None,
        }
    }
}

/// How a scenario misserves a companion resource's metadata, on whichever origin it places the
/// resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompanionFault {
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
    /// Faithful metadata that lists a companion resource of its own.
    Relisting,
}

impl RemoteScenario {
    /// Lists `companion` after the companion resources listed before it, and serves its metadata.
    ///
    /// # Panics
    ///
    /// `companion` has no kind, or the kind of a companion resource listed before it; or its
    /// metadata would lie where the document's protected resource metadata lies on either
    /// origin, or where that of a companion resource listed before it on its origin lies.
    #[must_use]
    pub fn accompanied_by(mut self, companion: Companion) -> Self {
        let apart = |listed: &Companion| {
            listed.kind != companion.kind && listed.metadata_at() != companion.metadata_at()
        };
        assert!(
            !companion.kind.is_empty()
                && well_known(&companion.path) != well_known(RESOURCE)
                && self.companions.iter().all(apart),
            "a companion resource has a kind of its own and its metadata at an address of its \
             own: {companion:?}"
        );
        self.companions.push(companion);
        self
    }

    /// Answers the document's protected resource metadata with `raw` as the member that lists
    /// the companion resources, in place of the list they make, each `{origin}` in a string of it
    /// read as the simulator's origin. The companion resources keep their own metadata.
    #[must_use]
    pub fn listing(mut self, raw: Value) -> Self {
        self.listing = Some(raw);
        self
    }

    /// Whether the scenario places metadata, or names its protected resource, on another origin,
    /// which a second port stands for.
    pub(super) fn places_elsewhere(&self) -> bool {
        matches!(
            self.metadata,
            Metadata::OnAnotherOrigin | Metadata::ResourceOnAnotherOrigin
        ) || self.companions.iter().any(|companion| companion.elsewhere)
    }
}

impl State {
    /// The identifier of `companion`: its path on the simulator's origin, or on the second port's
    /// for a resource placed there.
    fn identifier(&self, companion: &Companion) -> String {
        let origin = match &self.elsewhere {
            Some(elsewhere) if companion.elsewhere => elsewhere,
            _ => &self.origin,
        };
        format!("{origin}{}", companion.path)
    }

    /// The identifier that the document's protected resource metadata lists for the companion
    /// resource of `kind`.
    pub(super) fn companion_identifier(&self, kind: &str) -> Option<String> {
        self.scenario
            .companions
            .iter()
            .find(|companion| companion.kind == kind)
            .map(|companion| self.identifier(companion))
    }

    /// The origin on which the challenge names the document's protected resource metadata: the
    /// second port's for a scenario that places it there, the simulator's otherwise.
    fn metadata_origin(&self) -> &str {
        match &self.elsewhere {
            Some(elsewhere) if self.scenario.metadata == Metadata::OnAnotherOrigin => elsewhere,
            _ => &self.origin,
        }
    }

    /// The address at which the challenge names the document's protected resource metadata: on
    /// the origin where it lies, where RFC 9728 places it for the protected resource, or, for a
    /// scenario that names its protected resource on another origin, at the path where that
    /// scenario serves it.
    pub(super) fn metadata_address(&self) -> String {
        let path = self
            .path_naming_elsewhere()
            .unwrap_or_else(|| well_known(RESOURCE));
        format!("{}{path}", self.metadata_origin())
    }

    /// The path on the simulator's origin at which a scenario that names its protected resource
    /// on another origin serves the document's protected resource metadata: the well-known path,
    /// then `@` and the second port's host and port, then the protected resource's path, so that
    /// the metadata's address with the well-known path taken out names the resource on the second
    /// port's origin, the simulator's origin being its user information. Nothing for any other
    /// scenario.
    pub(super) fn path_naming_elsewhere(&self) -> Option<String> {
        let authority = self.elsewhere.as_deref()?.strip_prefix("http://")?;
        (self.scenario.metadata == Metadata::ResourceOnAnotherOrigin)
            .then(|| format!("{WELL_KNOWN}@{authority}{RESOURCE}"))
    }

    /// Adds to `metadata`, the document's protected resource metadata, the member that lists the
    /// companion resources: the value its scenario scripts, or else an object of `resource` and
    /// `kind` for each companion resource, in the order listed. A scenario with neither adds none.
    pub(super) fn list_companions(&self, metadata: &mut Value) {
        if let Some(raw) = &self.scenario.listing {
            metadata[COMPANION_RESOURCES] = with_origin(raw, &self.origin);
        } else if !self.scenario.companions.is_empty() {
            let listed = self.scenario.companions.iter().map(|companion| {
                json!({ "resource": self.identifier(companion), "kind": companion.kind })
            });
            metadata[COMPANION_RESOURCES] = Value::Array(listed.collect());
        }
    }

    /// The answer on the simulator's origin to `request`, where its whole target, the path and
    /// the query, is the address of the protected resource metadata of a companion resource
    /// there; nothing elsewhere.
    pub(super) fn companion_metadata(&self, request: &Request) -> Option<Response> {
        let companion = self.placed(&request.path, false)?;
        Some(self.metadata_answer(companion, request))
    }

    /// The answer to `request` at the address of the protected resource metadata of `companion`:
    /// that metadata as its scenario serves it, to a `GET`, or `not_found` where its scenario
    /// makes it unreadable.
    fn metadata_answer(&self, companion: &Companion, request: &Request) -> Response {
        match self.served(companion) {
            Some(metadata) => only("GET", request, || Response::json(200, &metadata)),
            None => refusal("not_found"),
        }
    }

    /// The scopes that `resource` accepts, if the scenario serves it: the document's protected
    /// resource, or a companion resource on the simulator's origin whose metadata names it, lists
    /// the simulator's issuer and lists a scope. They are the scopes its metadata lists in
    /// `scopes_supported`.
    pub(super) fn accepted(&self, resource: &str) -> Option<Vec<String>> {
        if resource == self.resource() {
            return Some(self.document_scopes());
        }
        let companions = self.scenario.companions.iter();
        companions
            .filter(|companion| !companion.elsewhere && self.identifier(companion) == resource)
            .find_map(Companion::vouched)
    }

    /// The companion resource whose protected resource metadata lies at `target` on the second
    /// port, if `elsewhere`, or on the simulator's origin otherwise.
    fn placed(&self, target: &str, elsewhere: bool) -> Option<&Companion> {
        self.scenario.companions.iter().find(|companion| {
            companion.elsewhere == elsewhere && target == well_known(&companion.path)
        })
    }

    /// The protected resource metadata of `companion` as its scenario serves it, or nothing for
    /// metadata it makes unreadable.
    fn served(&self, companion: &Companion) -> Option<Value> {
        let identifier = self.identifier(companion);
        let issuer = self.issuer();
        let mut metadata = json!({
            "resource": identifier,
            "authorization_servers": [issuer],
            "scopes_supported": companion.scopes,
            "bearer_methods_supported": ["header"],
        });
        match companion.fault {
            None => {}
            Some(CompanionFault::UnderAnotherIssuer) => {
                metadata["authorization_servers"] = json!([format!("{issuer}/elsewhere")]);
            }
            Some(CompanionFault::Misnamed) => {
                metadata["resource"] = json!(format!("{identifier}/elsewhere"));
            }
            Some(CompanionFault::Unnamed) => metadata = without(metadata, "resource"),
            Some(CompanionFault::Scopeless) => metadata = without(metadata, "scopes_supported"),
            Some(CompanionFault::EmptyScopes) => metadata["scopes_supported"] = json!([]),
            Some(CompanionFault::Unreadable) => return None,
            Some(CompanionFault::NotAnObject) => metadata = json!([metadata]),
            Some(CompanionFault::Relisting) => {
                let relisted = format!("{}/relisted", self.origin);
                metadata[COMPANION_RESOURCES] =
                    json!([{ "resource": relisted, "kind": "relisted" }]);
            }
        }
        Some(metadata)
    }
}

/// The answer of the second port, which stands for another origin: the metadata of each companion
/// resource placed there, as its scenario serves it, and of the document's protected resource for
/// a scenario that places it there; `not_found` to every other request.
pub(super) fn elsewhere(state: &mut State, request: &Request) -> Response {
    let moved = state.scenario.metadata == Metadata::OnAnotherOrigin
        && request.path == well_known(RESOURCE);
    if moved {
        let metadata = document_metadata(state, state.metadata_origin());
        return only("GET", request, || Response::json(200, &metadata));
    }
    match state.placed(&request.path, true) {
        Some(companion) => state.metadata_answer(companion, request),
        None => refusal("not_found"),
    }
}

/// Where RFC 9728 places the protected resource metadata of the resource at `path`, a path that
/// may end in a query: `/.well-known/oauth-protected-resource`, then the path without a
/// terminating `/`, then the query.
pub(super) fn well_known(path: &str) -> String {
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

#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;

    use super::{Companion, RESOURCE, RemoteScenario};

    #[test]
    fn a_scenario_lists_a_companion_resource_only_with_a_kind_and_a_metadata_address_of_its_own() {
        let listed =
            || RemoteScenario::accepting("key").accompanied_by(Companion::new("notes", &["notes"]));
        for refused in [
            Companion::new("", &["any"]),
            Companion::new("notes", &["any"]).at("/any"),
            Companion::new("any", &["any"]).at("/notes"),
            Companion::new("any", &["any"]).at("/notes/"),
            Companion::new("documents", &["documents"]).at(RESOURCE),
            Companion::new("documents", &["documents"])
                .at(RESOURCE)
                .elsewhere(),
        ] {
            let listing = catch_unwind(|| listed().accompanied_by(refused.clone()));
            assert!(listing.is_err(), "{refused:?}");
        }
        let apart = Companion::new("any", &["any"]).at("/notes").elsewhere();
        assert_eq!(listed().accompanied_by(apart).companions.len(), 2);
    }

    #[test]
    fn a_companion_resource_lies_only_at_a_path_that_begins_with_a_slash() {
        let refused = catch_unwind(|| Companion::new("notes", &["notes"]).at("notes/all"));
        let payload = refused.expect_err("a path without a leading `/` is refused");
        let message = payload.downcast_ref::<String>().map_or("", String::as_str);
        assert!(message.contains("\"notes/all\""), "{message}");
        let placed = Companion::new("notes", &["notes"]).at("/notes/all?view=mine");
        assert_eq!(placed.path, "/notes/all?view=mine");
    }
}

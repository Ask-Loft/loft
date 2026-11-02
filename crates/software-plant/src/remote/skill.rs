//! The skill a remote simulator publishes: its manifest at `/.well-known/loft-skill.json` on the
//! origin, and the package it names, each answered without a token.

use serde_json::json;

use super::problem::refusal;
use super::wire::sha256_hex;
use super::{Misstating, State};
use crate::http::Response;

/// Where the manifest lies, from the origin.
pub(super) const MANIFEST: &str = "/.well-known/loft-skill.json";

/// The path of the package that a skill named `name` is served under.
fn package_path(name: &str) -> String {
    format!("/skills/{name}.tar.gz")
}

/// The manifest, or `not_found` for a scenario that serves no skill.
pub(super) fn manifest(state: &State) -> Response {
    let Some(skill) = &state.scenario.skill else {
        return refusal("not_found");
    };
    Response::json(
        200,
        &json!({
            "name": skill.name,
            "version": skill.version,
            "package": package_path(&skill.name),
            "sha256": sha256_hex(&skill.package),
            "length": skill.package.len(),
        }),
    )
}

/// The package at `path`, if it is the one the manifest names: its bytes, or other bytes for a
/// tampering scenario.
pub(super) fn package(state: &State, path: &str) -> Option<Response> {
    let skill = state.scenario.skill.as_ref()?;
    if path != package_path(&skill.name) {
        return None;
    }
    let mut body = skill.package.clone();
    if state.scenario.misstating.contains(&Misstating::Package) {
        body.push(0);
    }
    Some(Response {
        status: 200,
        headers: vec![("Content-Type".to_owned(), "application/gzip".to_owned())],
        body,
    })
}

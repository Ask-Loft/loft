//! The contract suite of the skill a remote simulator publishes: its manifest on the origin and
//! the package it names, each answered without a token. No remote yet serves a skill, so each
//! scenario is asked of the simulator alone.

use serde_json::json;
use software_plant::remote::wire::sha256_hex;
use software_plant::remote::{Remote, RemoteScenario};

use crate::remote_contract::{Answer, assert_problem, call};

const KEY: &str = "01a18e30-0000-7000-8000-000000000020";

/// The bytes the simulator serves as a package: any bytes, since it reads none of them.
const PACKAGE: &[u8] = b"\x1f\x8b a package";

/// The package at `path` on the origin, fetched without a token: its status, its
/// `Content-Type` and its bytes, which the contract suite's `call` would read as JSON.
fn fetch(remote: &Remote, path: &str) -> (u16, Option<String>, Vec<u8>) {
    let mut response = crate::remote_contract::agent()
        .get(format!("{}{path}", remote.origin()))
        .call()
        .unwrap();
    let kind = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response.body_mut().read_to_vec().unwrap();
    (response.status().as_u16(), kind, body)
}

/// The skill manifest on the origin, asked for without a token.
fn manifest_of(remote: &Remote) -> Answer {
    call(
        "GET",
        &format!("{}/.well-known/loft-skill.json", remote.origin()),
        None,
        None,
    )
}

#[test]
fn the_skill_manifest_lies_on_the_origin_and_names_the_package_without_a_token() {
    let remote =
        Remote::start(RemoteScenario::accepting(KEY).skill("review", "1.2.0", PACKAGE.to_vec()));
    let answer = manifest_of(&remote);
    assert_eq!(answer.status, 200);
    assert_eq!(answer.header("content-type"), Some("application/json"));
    let manifest = answer.body;
    assert_eq!(manifest["name"], "review");
    assert_eq!(manifest["version"], "1.2.0");
    assert_eq!(manifest["sha256"], json!(sha256_hex(PACKAGE)));
    assert_eq!(manifest["length"], json!(PACKAGE.len()));
    let package = manifest["package"].as_str().unwrap();
    assert!(package.starts_with('/'), "{package}");
    // The address holds a path; the manifest is on its origin all the same.
    assert!(remote.address().contains("/documents/"));
    let under_address = call(
        "GET",
        &format!("{}/.well-known/loft-skill.json", remote.address()),
        None,
        None,
    );
    assert_ne!(under_address.status, 200, "only the origin serves it");

    let (status, kind, body) = fetch(&remote, package);
    assert_eq!(status, 200);
    assert_eq!(kind.as_deref(), Some("application/gzip"));
    assert_eq!(body, PACKAGE, "byte for byte");
}

#[test]
fn a_tampering_scenario_serves_other_bytes_and_one_without_a_skill_answers_not_found() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .skill("review", "1.2.0", PACKAGE.to_vec())
            .tampering(),
    );
    let manifest = manifest_of(&remote).body;
    let (status, _, body) = fetch(&remote, manifest["package"].as_str().unwrap());
    assert_eq!(status, 200);
    assert_ne!(body, PACKAGE);
    assert_eq!(
        manifest["sha256"],
        json!(sha256_hex(PACKAGE)),
        "the same manifest"
    );

    let remote = Remote::start(RemoteScenario::accepting(KEY));
    assert_problem(&manifest_of(&remote), 404, "not_found");
}

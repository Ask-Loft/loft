//! The contract suite of a failure a remote simulator scripts beside those of
//! `remote_contract_failures`: protected resource metadata on the simulator's own origin that
//! names the document's protected resource on another origin, which a second port of the loopback
//! interface stands for. A real remote is not asked to fail, so the scenario is asked of the
//! simulator alone.

use serde_json::json;
use software_plant::remote::{Metadata, Remote, RemoteScenario};
use ureq::http::Uri;

use crate::common::calls::call;
use crate::common::oauth::{KEY, challenge_at, parameter};
use crate::remote_contract::assert_problem;

/// Where protected resource metadata lies on its resource's origin, before the resource's path
/// (RFC 9728).
const WELL_KNOWN: &str = "/.well-known/oauth-protected-resource";

#[test]
fn a_scenario_naming_its_resource_on_another_origin_serves_metadata_naming_the_second_port() {
    let remote =
        Remote::start(RemoteScenario::accepting(KEY).metadata(Metadata::ResourceOnAnotherOrigin));
    let challenge = challenge_at(&remote.address());
    let named = parameter(&challenge, "resource_metadata").unwrap();
    // The challenge names the metadata on the simulator's origin, at
    // `/.well-known/oauth-protected-resource@<host>:<port>` followed by the resource's path.
    let authority = named
        .strip_prefix(&format!("{}{WELL_KNOWN}@", remote.origin()))
        .and_then(|rest| rest.strip_suffix("/documents"))
        .unwrap_or_else(|| panic!("{challenge}"));
    let elsewhere = format!("http://{authority}");
    let loopback = elsewhere.starts_with("http://127.0.0.1:");
    assert!(loopback && elsewhere != remote.origin(), "{challenge}");
    let answer = call("GET", &named, None, None);
    assert_eq!(answer.status, 200, "{answer:?}");
    let resource = named.replacen(WELL_KNOWN, "", 1);
    assert_eq!(
        answer.body,
        json!({
            "resource": resource,
            "authorization_servers": [remote.issuer()],
            "scopes_supported": ["documents"],
            "bearer_methods_supported": ["header"],
        })
    );
    // As a request's parser reads the resource, the simulator's origin is its user information,
    // and its host and port are those of the second port.
    let uri: Uri = resource.parse().unwrap();
    let host = uri.host().unwrap();
    let origin = format!("http://{host}:{}", uri.port_u16().unwrap());
    assert_eq!(origin, elsewhere, "{resource}");
    // The second port, asked nothing so far, records what it is asked.
    assert!(remote.requests_elsewhere().is_empty());
    let probe = call("GET", &format!("{elsewhere}/documents"), None, None);
    assert_problem(&probe, 404, "not_found");
    let asked: Vec<String> = remote
        .requests_elsewhere()
        .into_iter()
        .map(|request| request.path)
        .collect();
    assert_eq!(asked, ["/documents"]);
}

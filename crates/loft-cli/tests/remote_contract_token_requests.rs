//! The contract suite of the resource that a token request names. A code exchange or a refresh
//! names a single resource, by giving `resource` once: one that gives it more than once is refused
//! `invalid_target`, whatever code or refresh token it presents and in whichever order it names
//! its resources, and issues, rotates and revokes nothing; so is a code exchange that names no
//! resource, or a resource that no remote serves, which leaves its code to be exchanged once. A
//! refresh that names none, or a resource that no remote serves, has its scenario beside the other
//! refusals of a refresh, in `remote_contract_refreshes.rs`.
//!
//! Each scenario runs against the simulator and, on demand, against the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS`: run a real half with
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/<its name>/)'`. A real half of an
//! exchange registers a client of its own and presents a code never issued, since a live code
//! needs a person to consent. The real half of the refresh spends the sign-in that
//! `loft remote login` kept in the `loft/remotes.json` that `LOFT_REMOTE_CONTRACT_SIGN_IN` names,
//! so run it alone, after a fresh sign-in. A real half learns the companion resources from the
//! remote's own metadata and shares its scenario with its simulator half, so that the two cannot
//! drift. The simulator alone is asked how many refreshes it accepted, which access tokens a
//! resource admits, and how a scenario that refuses every exchange or lapses answers.

use serde_json::{Value, json};
use software_plant::remote::{Companion, Remote, RemoteScenario};

use crate::common::oauth::{
    KEY, UNKNOWN_CODE, UNSERVED, assert_oauth_error, assert_oauth_error_for, coded, coded_for,
    discover, exchange, exchange_for, refresh_for, registered,
};
use crate::common::resources::{Signed, companions_at, companions_of, kept, signed_over};
use crate::remote_contract_refreshes::UNREFRESHED;

/// A code that no remote issued, beside the words that name it in a failure.
const NEVER_ISSUED: (&str, &str) = ("a code never issued", UNKNOWN_CODE);

/// The resources of each token request that names `resource` more than once, of a remote whose
/// own protected resource is `own` and which lists `companions`: `own` and a resource that no
/// remote serves, in either order; `own` twice; and `own` with each companion resource.
fn several<'a>(own: &'a str, companions: &'a [(String, String)]) -> Vec<Vec<&'a str>> {
    let mut named = vec![vec![own, UNSERVED], vec![UNSERVED, own], vec![own, own]];
    let paired = companions
        .iter()
        .map(|(companion, _)| vec![own, companion.as_str()]);
    named.extend(paired);
    named
}

/// Refuses `invalid_target` each code exchange by `client` with the authorization server whose
/// metadata is `server` that names `resource` once for each of the resources of one of
/// `requests`, whichever of `codes` it presents: each code beside the words that name it in a
/// failure, which never shows the code itself.
fn refuses_each_exchange(
    server: &Value,
    client: &str,
    requests: &[Vec<&str>],
    codes: &[(&str, &str)],
) {
    for (which, code) in codes {
        for named in requests {
            let answer = exchange_for(server, client, named, code);
            let asked = format!("an exchange of {which} that names {named:?}");
            assert_oauth_error_for(&answer, "invalid_target", &asked);
        }
    }
}

/// Refuses `invalid_target` each code exchange by `client`, at the remote of the document at
/// `address`, that names `resource` more than once, as [`several`] lists them, whichever of
/// `codes` it presents.
fn refuses_an_exchange_that_names_resource_more_than_once(
    address: &str,
    client: &str,
    codes: &[(&str, &str)],
) {
    let (metadata, server) = discover(address);
    let own = metadata["resource"]
        .as_str()
        .expect("protected resource metadata that names its resource");
    let companions = companions_of(&metadata);
    refuses_each_exchange(&server, client, &several(own, &companions), codes);
}

/// Refuses `invalid_target` each code exchange by `client` with the authorization server whose
/// metadata is `server` that names no resource, or a resource that no remote serves, whichever of
/// `codes` it presents.
fn refuses_an_exchange_that_names_no_resource_or_one_it_does_not_serve(
    server: &Value,
    client: &str,
    codes: &[(&str, &str)],
) {
    refuses_each_exchange(server, client, &[vec![], vec![UNSERVED]], codes);
}

#[test]
fn an_exchange_that_names_no_resource_or_one_the_remote_does_not_serve_is_refused_and_leaves_its_code_to_be_exchanged_once()
 {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let (server, client, own, code) = coded(&remote);
    let codes = [("the code issued", code.as_str()), NEVER_ISSUED];
    refuses_an_exchange_that_names_no_resource_or_one_it_does_not_serve(&server, &client, &codes);
    // No refusal used the code up: it is exchanged once.
    let answer = exchange(&server, &client, &own, &code);
    assert_eq!(answer.status, 200, "{answer:?}");
    assert_oauth_error(&exchange(&server, &client, &own, &code), "invalid_grant");
    // Presented again once exchanged, the code is refused `invalid_target` all the same.
    let exchanged = [("the code once exchanged", code.as_str())];
    refuses_an_exchange_that_names_no_resource_or_one_it_does_not_serve(
        &server, &client, &exchanged,
    );
    // A scenario that refuses every exchange judges its resources first, and refuses one that
    // names a single resource it serves with its own error.
    let refusing = RemoteScenario::accepting(KEY).refusing_exchange("unauthorized_client");
    let refusing = Remote::start(refusing);
    let (server, client, own, code) = coded(&refusing);
    let issued = [("the code issued", code.as_str())];
    refuses_an_exchange_that_names_no_resource_or_one_it_does_not_serve(&server, &client, &issued);
    let refused = exchange(&server, &client, &own, &code);
    assert_oauth_error(&refused, "unauthorized_client");
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_refuses_an_exchange_that_names_no_resource_or_one_it_does_not_serve() {
    // Registers a client of its own and presents a code never issued: a live code needs a person
    // to consent.
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let (_, server) = discover(&address);
    let client = registered(&server);
    let codes = [NEVER_ISSUED];
    refuses_an_exchange_that_names_no_resource_or_one_it_does_not_serve(&server, &client, &codes);
}

#[test]
fn an_exchange_that_names_resource_more_than_once_is_refused_and_leaves_its_code_to_be_exchanged_once()
 {
    let notes = Companion::new("notes", &["notes"]);
    let remote = Remote::start(RemoteScenario::accepting(KEY).accompanied_by(notes));
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let (server, client, code) = coded_for(&remote, &[&own, &notes], "documents notes");
    let codes = [("the code issued", code.as_str()), NEVER_ISSUED];
    refuses_an_exchange_that_names_resource_more_than_once(&remote.address(), &client, &codes);
    // No refusal used the code up: it is exchanged once, for one resource it names.
    let answer = exchange(&server, &client, &notes, &code);
    assert_eq!(answer.status, 200, "{answer:?}");
    assert_eq!(answer.body["resource"], json!(notes));
    assert_oauth_error(&exchange(&server, &client, &own, &code), "invalid_grant");
    // Presented again once exchanged, the code is refused `invalid_target` all the same.
    let exchanged = [("the code once exchanged", code.as_str())];
    refuses_an_exchange_that_names_resource_more_than_once(&remote.address(), &client, &exchanged);
    // A scenario that refuses every exchange judges its resources first.
    let refusing = RemoteScenario::accepting(KEY).refusing_exchange("unauthorized_client");
    let refusing = Remote::start(refusing);
    let (server, client, own, code) = coded(&refusing);
    let issued = [("the code issued", code.as_str())];
    refuses_an_exchange_that_names_resource_more_than_once(&refusing.address(), &client, &issued);
    let refused = exchange(&server, &client, &own, &code);
    assert_oauth_error(&refused, "unauthorized_client");
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_refuses_an_exchange_that_names_resource_more_than_once() {
    // Registers a client of its own and presents a code never issued: a live code needs a person
    // to consent.
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let (_, server) = discover(&address);
    let client = registered(&server);
    refuses_an_exchange_that_names_resource_more_than_once(&address, &client, &[NEVER_ISSUED]);
}

/// Refuses `invalid_target` each refresh of `signed` that names `resource` more than once, as
/// [`several`] lists them, whether its refresh token is live, rotated or one never issued, and
/// rotates and revokes nothing: its live refresh token then refreshes the sign-in's resource, and
/// that answer's refresh token after it.
fn refuses_a_refresh_that_names_resource_more_than_once(signed: &Signed) {
    let companions = companions_at(&signed.address);
    // `which` names the refresh token in a failure, which never shows the token itself.
    let refused = |which: &str, token: &str| {
        for named in several(&signed.resource, &companions) {
            let answer = refresh_for(&signed.server(), &signed.client, &named, token);
            let asked = format!("a refresh of {which} that names {named:?}");
            assert_oauth_error_for(&answer, "invalid_target", &asked);
        }
    };
    let live = signed.refresh_token.as_str();
    refused("the live refresh token", live);
    let never = "a-refresh-token-never-issued";
    refused("a refresh token never issued", never);
    let rotated = signed.refresh(&signed.resource, live);
    let unrotated = format!("a refused refresh rotates nothing ({UNREFRESHED})");
    assert_eq!(rotated.status, 200, "{unrotated}: {rotated:?}");
    refused("the rotated refresh token", live);
    let successor = rotated.body["refresh_token"].as_str().unwrap();
    let next = signed.refresh(&signed.resource, successor);
    let unrevoked = "a refused refresh revokes nothing";
    assert_eq!(next.status, 200, "{unrevoked}: {next:?}");
}

#[test]
fn a_refresh_that_names_resource_more_than_once_is_refused_and_rotates_nothing() {
    let scenario =
        RemoteScenario::accepting(KEY).accompanied_by(Companion::new("notes", &["notes"]));
    let remote = Remote::start(scenario.clone());
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let signed = signed_over(&remote, &[&own, &notes], "documents notes");
    refuses_a_refresh_that_names_resource_more_than_once(&signed);
    // The two refreshes it accepted are all that rotated, and the sign-in was not revoked.
    assert_eq!(remote.refreshes(), 2);
    assert!(remote.admits_at(&own, &signed.access_token));
    // A lapsing scenario judges a refresh's resources before its lapse.
    let lapsing = Remote::start(scenario.lapsing());
    let (own, notes) = (lapsing.resource(), lapsing.companion("notes"));
    let signed = signed_over(&lapsing, &[&own, &notes], "documents notes");
    let token = signed.refresh_token.as_str();
    let companions = companions_at(&lapsing.address());
    for named in several(&own, &companions) {
        let answer = refresh_for(&signed.server(), &signed.client, &named, token);
        assert_oauth_error(&answer, "invalid_target");
    }
    assert_oauth_error(&signed.refresh(&own, token), "invalid_grant");
    assert_eq!(lapsing.refreshes(), 0);
}

#[test]
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_refuses_a_refresh_that_names_resource_more_than_once_and_rotates_nothing() {
    // Needs a sign-in that any `loft remote login` kept for the address, which nothing has
    // refreshed since.
    refuses_a_refresh_that_names_resource_more_than_once(&kept());
}

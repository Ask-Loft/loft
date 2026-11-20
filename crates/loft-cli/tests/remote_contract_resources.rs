//! The contract suite of a sign-in over a remote's companion resources:
//! - the companion resources that its protected resource metadata lists, and each one's own
//!   metadata;
//! - one authorization request for every resource, and a code exchanged for one of them by the
//!   client it was issued to alone.
//!
//! The refreshes of such a sign-in have a suite of their own, `remote_contract_refreshes.rs`; so
//! does a code exchange or a refresh that names more than one resource,
//! `remote_contract_token_requests.rs`, and an authorization request that the remote refuses
//! before any person consents, `remote_contract_refusals.rs`.
//!
//! Each scenario runs against the simulator and, on demand, against the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS`: run a real half with
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/<its name>/)'`. A scenario that
//! needs a person to consent, or asks which access tokens a companion resource admits, runs
//! against the simulator alone, and so does such a part of a scenario: the real half of the
//! refusal of a code registers a client of its own and presents a code that no remote issued,
//! since another client's code needs a person to consent. A real half learns the companion
//! resources from the remote's own metadata, and shares its scenario with its simulator half, so
//! that the two cannot drift. A real half whose premise does not hold fails with a message that
//! names the premise; none passes without checking.

use std::iter;

use serde_json::{Value, json};
use software_plant::remote::wire::value;
use software_plant::remote::{Companion, CompanionFault, Remote, RemoteScenario};

use crate::common::oauth::{
    KEY, UNKNOWN_CODE, UNSERVED, assert_oauth_error, authorize_for, challenge_at, coded_for,
    companion_metadata, discover, exchange, exchange_for, fetch, location, metadata, parameter,
    refresh, registered,
};
use crate::common::resources::{COMPANION_RESOURCES, companions_of, head_with, origin_of};
use crate::remote_contract::assert_problem;

/// What a remote's challenge, and the protected resource metadata it names, say of the remote's
/// companion resources.
struct Listing {
    /// The challenge of a request without a token.
    challenge: String,
    /// The protected resource metadata that the challenge names.
    metadata: Value,
    /// Each companion resource that the metadata lists, as its `resource` and its `kind`.
    companions: Vec<(String, String)>,
}

/// The listing of the document at `address`, held to what a remote promises of it: its challenge
/// names metadata on the address's origin that lists each scope the challenge names; and each
/// companion resource has a kind, lies on that origin, is not the remote's own, is listed once,
/// and vouches for itself under the remote's issuer.
fn listing_as_promised(address: &str) -> Listing {
    let origin = origin_of(address);
    let (challenge, metadata, _) = fetch(address);
    let named = parameter(&challenge, "resource_metadata").expect("a challenge naming metadata");
    assert_eq!(origin_of(&named), origin, "{challenge}");
    let supported = metadata["scopes_supported"].as_array().cloned();
    let supported = supported.unwrap_or_default();
    for scope in parameter(&challenge, "scope")
        .iter()
        .flat_map(|s| s.split(' '))
    {
        assert!(supported.contains(&json!(scope)), "{scope}: {metadata}");
    }
    let companions = companions_of(&metadata);
    for (at, (resource, kind)) in companions.iter().enumerate() {
        assert!(!kind.is_empty(), "{resource} names a kind");
        assert_eq!(origin_of(resource), origin, "{kind} lies on the origin");
        assert_ne!(
            metadata["resource"],
            json!(resource),
            "{kind} is the remote's"
        );
        let earlier = &companions[..at];
        let repeated = earlier.iter().any(|(r, k)| r == resource || k == kind);
        assert!(!repeated, "{kind} is listed twice");
        vouches_for_itself(resource, kind, &metadata["authorization_servers"][0]);
    }
    Listing {
        challenge,
        metadata,
        companions,
    }
}

/// Requires the protected resource metadata of the companion resource `resource`, of `kind`, at
/// the address RFC 9728 derives from it, to be a JSON object that names it, `issuer` and a scope.
fn vouches_for_itself(resource: &str, kind: &str, issuer: &Value) {
    let answer = companion_metadata(resource);
    let metadata = &answer.body;
    assert_eq!(answer.status, 200, "{kind}: {metadata}");
    assert!(metadata.is_object(), "{kind}: {metadata}");
    assert_eq!(metadata["resource"], json!(resource), "{kind}: {metadata}");
    let servers = metadata["authorization_servers"].as_array().cloned();
    let servers = servers.unwrap_or_default();
    assert!(
        servers.contains(issuer),
        "{kind} names {issuer}: {metadata}"
    );
    let scopes = metadata["scopes_supported"].as_array().map_or(0, Vec::len);
    assert!(scopes > 0, "{kind} lists a scope: {metadata}");
}

/// Whether `value` holds each of `fields`, in their order.
fn present(value: &Value, fields: &[&str]) -> Vec<bool> {
    fields
        .iter()
        .map(|field| value.get(*field).is_some())
        .collect()
}

#[test]
fn the_metadata_lists_each_companion_resource_and_each_names_itself_and_the_issuer() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .accompanied_by(Companion::new("notes", &["notes", "notes.archive"]))
            .accompanied_by(Companion::new("inbox", &["inbox"]).at("/mail/inbox?view=all")),
    );
    let listing = listing_as_promised(&remote.address());
    let named = parameter(&listing.challenge, "resource_metadata").unwrap();
    assert_eq!(origin_of(&named), remote.origin());
    let scope = parameter(&listing.challenge, "scope");
    assert_eq!(scope.as_deref(), Some("documents"));
    assert_eq!(listing.metadata["scopes_supported"], json!(["documents"]));
    let (notes, inbox) = (remote.companion("notes"), remote.companion("inbox"));
    assert_eq!(notes, format!("{}/notes", remote.origin()));
    assert_eq!(inbox, format!("{}/mail/inbox?view=all", remote.origin()));
    assert_eq!(
        listing.metadata[COMPANION_RESOURCES],
        json!([
            { "resource": notes, "kind": "notes" },
            { "resource": inbox, "kind": "inbox" },
        ])
    );
    let issuer = &listing.metadata["authorization_servers"];
    assert_eq!(issuer, &json!([remote.issuer()]));
    for (resource, scopes) in [
        (&notes, json!(["notes", "notes.archive"])),
        (&inbox, json!(["inbox"])),
    ] {
        let answer = companion_metadata(resource);
        assert_eq!(answer.status, 200, "{answer:?}");
        assert_eq!(answer.header("content-type"), Some("application/json"));
        assert_eq!(
            answer.body,
            json!({
                "resource": resource,
                "authorization_servers": issuer,
                "scopes_supported": scopes,
                "bearer_methods_supported": ["header"],
            })
        );
    }
    let paths: Vec<String> = remote.requests().into_iter().map(|r| r.path).collect();
    let inbox_metadata = "/.well-known/oauth-protected-resource/mail/inbox?view=all";
    assert!(paths.iter().any(|path| path == inbox_metadata), "{paths:?}");

    let unaccompanied = Remote::start(RemoteScenario::accepting(KEY));
    let listing = listing_as_promised(&unaccompanied.address());
    let member = listing.metadata.get(COMPANION_RESOURCES);
    assert!(member.is_none(), "{}", listing.metadata);
    assert_eq!(listing.companions, Vec::<(String, String)>::new());
}

#[test]
fn the_metadata_of_a_companion_resource_whose_path_ends_in_a_slash_lies_at_the_path_without_it() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .accompanied_by(Companion::new("inbox", &["inbox"]).at("/mail/inbox/?view=all"))
            .accompanied_by(Companion::new("home", &["home"]).at("/")),
    );
    for (kind, path, metadata) in [
        (
            "inbox",
            "/mail/inbox/?view=all",
            "/.well-known/oauth-protected-resource/mail/inbox?view=all",
        ),
        ("home", "/", "/.well-known/oauth-protected-resource"),
    ] {
        let identifier = remote.companion(kind);
        assert_eq!(identifier, format!("{}{path}", remote.origin()));
        let answer = companion_metadata(&identifier);
        assert_eq!(answer.status, 200, "{kind}: {answer:?}");
        assert_eq!(answer.body["resource"], json!(identifier), "{kind}");
        let paths: Vec<String> = remote.requests().into_iter().map(|r| r.path).collect();
        assert!(paths.iter().any(|at| at == metadata), "{kind}: {paths:?}");
    }
}

#[test]
fn a_companion_resource_at_the_documents_path_with_a_query_has_metadata_of_its_own() {
    let gone = Companion::new("gone", &["gone"]).at("/documents?view=gone");
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .accompanied_by(Companion::new("notes", &["notes"]).at("/documents?view=notes"))
            .accompanied_by(gone.faulty(CompanionFault::Unreadable)),
    );
    let (own, notes, gone) = (
        remote.resource(),
        remote.companion("notes"),
        remote.companion("gone"),
    );
    assert_eq!(notes, format!("{own}?view=notes"));
    let (listed, _) = metadata(&remote);
    assert_eq!(listed["resource"], json!(own));
    let issuer = &listed["authorization_servers"];
    assert_eq!(issuer, &json!([remote.issuer()]));
    assert_eq!(
        listed[COMPANION_RESOURCES],
        json!([
            { "resource": notes, "kind": "notes" },
            { "resource": gone, "kind": "gone" },
        ])
    );
    let answer = companion_metadata(&notes);
    assert_eq!(answer.status, 200, "{answer:?}");
    assert_eq!(
        answer.body,
        json!({
            "resource": notes,
            "authorization_servers": issuer,
            "scopes_supported": ["notes"],
            "bearer_methods_supported": ["header"],
        })
    );
    let paths: Vec<String> = remote.requests().into_iter().map(|r| r.path).collect();
    let at = "/.well-known/oauth-protected-resource/documents?view=notes";
    assert!(paths.iter().any(|path| path == at), "{paths:?}");
    assert_problem(&companion_metadata(&gone), 404, "not_found");
    let (server, client, code) = coded_for(&remote, &[&own, &notes], "documents notes");
    let answer = exchange(&server, &client, &notes, &code);
    assert_eq!(answer.status, 200, "{answer:?}");
    assert_eq!(answer.body["resource"], json!(notes));
    assert_eq!(answer.body["scope"], "notes");
    let access = answer.body["access_token"].as_str().unwrap();
    assert!(remote.admits_at(&notes, access), "{notes} admits its token");
    assert!(
        !remote.admits_at(&own, access),
        "{own} admits no token of {notes}"
    );
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_lists_its_companion_resources_in_the_shape_the_simulator_does() {
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let real = listing_as_promised(&address);
    let scenario = RemoteScenario::accepting(KEY);
    let remote = Remote::start(if real.companions.is_empty() {
        scenario
    } else {
        scenario.accompanied_by(Companion::new("notes", &["notes"]))
    });
    let simulated = listing_as_promised(&remote.address());
    let fields = [
        "resource",
        "authorization_servers",
        "scopes_supported",
        COMPANION_RESOURCES,
    ];
    assert_eq!(
        present(&real.metadata, &fields),
        present(&simulated.metadata, &fields)
    );
    let fields = [
        "resource",
        "authorization_servers",
        "scopes_supported",
        "bearer_methods_supported",
    ];
    let shape = |resource: &str| present(&companion_metadata(resource).body, &fields);
    let expected = simulated
        .companions
        .first()
        .map(|(resource, _)| shape(resource));
    for (resource, kind) in &real.companions {
        assert_eq!(Some(shape(resource)), expected, "{kind}");
    }
}

#[test]
fn one_authorization_request_names_every_resource_and_its_code_is_exchanged_for_any_one_of_them() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .accompanied_by(Companion::new(
                "notes",
                &["notes", "notes.archive", "notes.share"],
            ))
            .accompanied_by(Companion::new("inbox", &["inbox"])),
    );
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let inbox = remote.companion("inbox");
    let listed = companion_metadata(&notes).body["scopes_supported"].clone();
    assert_eq!(listed, json!(["notes", "notes.archive", "notes.share"]));
    let asked = [own.as_str(), notes.as_str()];
    let scope = "documents notes notes.archive";
    let (server, client, first) = coded_for(&remote, &asked, scope);
    let code = || {
        let answer = authorize_for(&server, &client, &asked, scope);
        value(&location(&answer), "code").unwrap().to_owned()
    };
    // A resource the scenario does not serve, or none, is refused before the code is read, and
    // before a scenario that refuses every exchange refuses it.
    for (named, presented) in [
        (vec![UNSERVED], first.as_str()),
        (vec![UNSERVED], "an-unknown-code"),
        (Vec::new(), "an-unknown-code"),
    ] {
        let refused = exchange_for(&server, &client, &named, presented);
        assert_oauth_error(&refused, "invalid_target");
    }
    let refusing = Remote::start(RemoteScenario::accepting(KEY).refusing_exchange("invalid_grant"));
    let (_, refusing_server) = metadata(&refusing);
    let refused = exchange(&refusing_server, &client, UNSERVED, "an-unknown-code");
    assert_oauth_error(&refused, "invalid_target");
    let unheld = exchange(&server, &client, &inbox, &code());
    assert_oauth_error(&unheld, "invalid_target");
    let exchanged = |resource: &str| {
        let answer = exchange(&server, &client, resource, &code());
        assert_eq!(answer.status, 200, "{answer:?}");
        assert_eq!(answer.body["resource"], json!(resource));
        let access = answer.body["access_token"].as_str().unwrap();
        for other in [&own, &notes, &inbox] {
            let admitted = remote.admits_at(other, access);
            assert_eq!(
                admitted,
                other.as_str() == resource,
                "{resource} at {other}"
            );
        }
        answer.body
    };
    assert_eq!(exchanged(&own)["scope"], "documents");
    let answer = exchanged(&notes);
    assert_eq!(answer["scope"], "notes notes.archive");
    let challenge = challenge_at(&remote.address());
    let at_the_document = head_with(&remote.address(), answer["access_token"].as_str().unwrap());
    assert_problem(&at_the_document, 401, "unauthenticated");
    let challenged = at_the_document.header("www-authenticate");
    assert_eq!(challenged, Some(challenge.as_str()));
    let successor = answer["refresh_token"].as_str().unwrap();
    let refreshed = refresh(&server, &client, &own, successor);
    assert_eq!(refreshed.status, 200, "{refreshed:?}");
    assert_eq!(refreshed.body["scope"], "documents");
}

/// Refuses `invalid_grant` each exchange, by a client registered with the remote of the document
/// at `address`, of a code that no remote issued, for the remote's own protected resource and for
/// each companion resource it lists.
fn refuses_an_unknown_code(address: &str) {
    let (metadata, server) = discover(address);
    let client = registered(&server);
    let own = metadata["resource"]
        .as_str()
        .expect("protected resource metadata that names its resource");
    let companions = companions_of(&metadata);
    let resources = iter::once(own).chain(companions.iter().map(|(at, _)| at.as_str()));
    for resource in resources {
        let unknown = exchange(&server, &client, resource, UNKNOWN_CODE);
        let refused = (unknown.status, &unknown.body["error"]);
        assert_eq!(
            refused,
            (400, &json!("invalid_grant")),
            "{resource}: {unknown:?}"
        );
    }
}

#[test]
fn an_unknown_code_or_one_another_client_presents_is_refused_for_any_resource_the_remote_serves() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY).accompanied_by(Companion::new("notes", &["notes"])),
    );
    refuses_an_unknown_code(&remote.address());
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let (server, client, code) = coded_for(&remote, &[&own], "documents");
    let other = registered(&server);
    // The remote serves `notes`, which the code's authorization request did not name: the code
    // is judged first, so that another client's is refused `invalid_grant` for `notes` too, not
    // `invalid_target`.
    for resource in [&own, &notes] {
        assert_oauth_error(&exchange(&server, &other, resource, &code), "invalid_grant");
    }
    let answer = exchange(&server, &client, &own, &code);
    let kept = "a refused exchange leaves the code to its client";
    assert_eq!(answer.status, 200, "{kept}: {answer:?}");
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_refuses_an_unknown_code_for_any_resource_it_serves() {
    // Registers a client of its own. A code that another client presents needs a person to
    // consent to the first client's request, so that part runs against the simulator alone.
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    refuses_an_unknown_code(&address);
}

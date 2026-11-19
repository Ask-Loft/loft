//! The contract suite of a sign-in over a remote's companion resources:
//! - the companion resources that its protected resource metadata lists, and each one's own
//!   metadata;
//! - one authorization request for every resource, and a code exchanged for one of them.
//!
//! The refreshes of such a sign-in have a suite of their own, `remote_contract_refreshes.rs`.
//!
//! Each scenario runs against the simulator and, on demand, against the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS`: run a real half with
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/<its name>/)'`. A scenario that
//! needs a person to consent, or asks which access tokens a companion resource admits, runs
//! against the simulator alone. A real half learns the companion resources from the remote's own
//! metadata, and shares its scenario with its simulator half, so that the two cannot drift. A
//! real half whose premise does not hold fails with a message that names the premise; none
//! passes without checking.

use serde_json::{Value, json};
use software_plant::remote::wire::value;
use software_plant::remote::{Companion, Remote, RemoteScenario};

use crate::common::calls::Answer;
use crate::common::oauth::{
    KEY, assert_oauth_error, authorize, authorize_for, coded_for, companion_metadata, exchange,
    exchange_for, fetch, location, metadata, parameter, refresh, registered,
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
/// companion resource lies on that origin, is not the remote's own, is listed once, and vouches
/// for itself under the remote's issuer.
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
    let companions = companions_of(address);
    for (at, (resource, kind)) in companions.iter().enumerate() {
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
        assert_eq!(answer.status, 200, "{:?}", answer.body);
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
        assert_eq!(answer.status, 200, "{kind}: {:?}", answer.body);
        assert_eq!(answer.body["resource"], json!(identifier), "{kind}");
        let paths: Vec<String> = remote.requests().into_iter().map(|r| r.path).collect();
        assert!(paths.iter().any(|at| at == metadata), "{kind}: {paths:?}");
    }
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
    let elsewhere = format!("{}/elsewhere", remote.origin());
    for (named, presented) in [
        (vec![elsewhere.as_str()], first.as_str()),
        (vec![elsewhere.as_str()], "an-unknown-code"),
        (Vec::new(), "an-unknown-code"),
    ] {
        let refused = exchange_for(&server, &client, &named, presented);
        assert_oauth_error(&refused, "invalid_target");
    }
    let refusing = Remote::start(RemoteScenario::accepting(KEY).refusing_exchange("invalid_grant"));
    let (_, refusing_server) = metadata(&refusing);
    let foreign = format!("{}/elsewhere", refusing.origin());
    let refused = exchange(&refusing_server, &client, &foreign, "an-unknown-code");
    assert_oauth_error(&refused, "invalid_target");
    let unheld = exchange(&server, &client, &inbox, &code());
    assert_oauth_error(&unheld, "invalid_target");
    let exchanged = |resource: &str| {
        let answer = exchange(&server, &client, resource, &code());
        assert_eq!(answer.status, 200, "{:?}", answer.body);
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
    let companion = exchanged(&notes);
    assert_eq!(companion["scope"], "notes notes.archive");
    let (challenge, _, _) = fetch(&remote.address());
    let at_the_document = head_with(
        &remote.address(),
        companion["access_token"].as_str().unwrap(),
    );
    assert_problem(&at_the_document, 401, "unauthenticated");
    let challenged = at_the_document.header("www-authenticate");
    assert_eq!(challenged, Some(challenge.as_str()));
    let successor = companion["refresh_token"].as_str().unwrap();
    let refreshed = refresh(&server, &client, &own, successor);
    assert_eq!(refreshed.status, 200, "{:?}", refreshed.body);
    assert_eq!(refreshed.body["scope"], "documents");
}

#[test]
fn an_authorization_request_is_refused_for_a_resource_or_a_scope_the_remote_does_not_serve() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY).accompanied_by(Companion::new("notes", &["notes"])),
    );
    let (_, server) = metadata(&remote);
    let client = registered(&server);
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let listed = companion_metadata(&notes).body["scopes_supported"].clone();
    assert_eq!(listed, json!(["notes"]));
    let elsewhere = format!("{}/elsewhere", remote.origin());
    let error = |answer: &Answer| value(&location(answer), "error").map(str::to_owned);
    for (resources, scope, refused) in [
        (vec![], "documents", "invalid_target"),
        (
            vec![own.as_str(), elsewhere.as_str()],
            "documents",
            "invalid_target",
        ),
        (vec![own.as_str()], "", "invalid_scope"),
        (vec![own.as_str()], "documents documents", "invalid_scope"),
        (vec![own.as_str()], "documents profile", "invalid_scope"),
        (vec![own.as_str()], "documents notes", "invalid_scope"),
        (
            vec![own.as_str(), notes.as_str()],
            "documents",
            "invalid_target",
        ),
    ] {
        let answer = authorize_for(&server, &client, &resources, scope);
        let named = format!("{resources:?} asking {scope:?}");
        assert_eq!(error(&answer).as_deref(), Some(refused), "{named}");
    }
    // A parameter sent without a value is read as one left out (RFC 6749, section 3.1).
    for (response_type, refused) in [
        ("", "invalid_request"),
        ("token", "unsupported_response_type"),
    ] {
        let answer = authorize(&server, &client, &own, &[("response_type", response_type)]);
        assert_eq!(
            error(&answer).as_deref(),
            Some(refused),
            "{response_type:?}"
        );
    }
    let unaccompanied = Remote::start(RemoteScenario::accepting(KEY));
    for remote in [&remote, &unaccompanied] {
        let own = remote.resource();
        let (server, client, code) = coded_for(remote, &[&own], "documents");
        let answer = exchange(&server, &client, &own, &code);
        assert_eq!(answer.status, 200, "{:?}", answer.body);
        assert_eq!(answer.body["resource"], json!(own));
    }
}

//! The contract suite of a sign-in over a remote's related resources:
//! - the related resources that its protected resource metadata lists, and each one's own
//!   metadata.
//!
//! Each scenario runs against the simulator and, on demand, against the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS`: run a real half with
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/<its name>/)'`. A real half learns
//! the related resources from the remote's own metadata, and shares its scenario with its
//! simulator half, so that the two cannot drift. A real half whose premise does not hold fails
//! with a message that names the premise; none passes without checking.

use serde_json::{Value, json};
use software_plant::remote::{Related, Remote, RemoteScenario};

use crate::common::oauth::{KEY, fetch, parameter, related_metadata};
use crate::common::resources::{RELATED_RESOURCES, origin_of, related_of};

/// What a remote's challenge, and the protected resource metadata it names, say of the remote's
/// related resources.
struct Listing {
    /// The challenge of a request without a token.
    challenge: String,
    /// The protected resource metadata that the challenge names.
    metadata: Value,
    /// Each related resource that the metadata lists, as its `resource` and its `kind`.
    related: Vec<(String, String)>,
}

/// The listing of the document at `address`, held to what a remote promises of it: its challenge
/// names metadata on the address's origin that lists each scope the challenge names; and each
/// related resource lies on that origin, is not the remote's own, is listed once, and vouches for
/// itself under the remote's issuer.
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
    let related = related_of(address);
    for (at, (resource, kind)) in related.iter().enumerate() {
        assert_eq!(origin_of(resource), origin, "{kind} lies on the origin");
        assert_ne!(
            metadata["resource"],
            json!(resource),
            "{kind} is the remote's"
        );
        let earlier = &related[..at];
        let repeated = earlier.iter().any(|(r, k)| r == resource || k == kind);
        assert!(!repeated, "{kind} is listed twice");
        vouches_for_itself(resource, kind, &metadata["authorization_servers"][0]);
    }
    Listing {
        challenge,
        metadata,
        related,
    }
}

/// Requires the protected resource metadata of the related resource `resource`, of `kind`, at
/// the address RFC 9728 derives from it, to be a JSON object that names it, `issuer` and a scope.
fn vouches_for_itself(resource: &str, kind: &str, issuer: &Value) {
    let answer = related_metadata(resource);
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
fn the_metadata_lists_each_related_resource_and_each_names_itself_and_the_issuer() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .relating(Related::new("notes", &["notes", "notes.archive"]))
            .relating(Related::new("inbox", &["inbox"]).at("/mail/inbox?view=all")),
    );
    let listing = listing_as_promised(&remote.address());
    let named = parameter(&listing.challenge, "resource_metadata").unwrap();
    assert_eq!(origin_of(&named), remote.origin());
    let scope = parameter(&listing.challenge, "scope");
    assert_eq!(scope.as_deref(), Some("documents"));
    assert_eq!(listing.metadata["scopes_supported"], json!(["documents"]));
    let (notes, inbox) = (remote.related("notes"), remote.related("inbox"));
    assert_eq!(notes, format!("{}/notes", remote.origin()));
    assert_eq!(inbox, format!("{}/mail/inbox?view=all", remote.origin()));
    assert_eq!(
        listing.metadata[RELATED_RESOURCES],
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
        let answer = related_metadata(resource);
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

    let unrelated = Remote::start(RemoteScenario::accepting(KEY));
    let listing = listing_as_promised(&unrelated.address());
    let member = listing.metadata.get(RELATED_RESOURCES);
    assert!(member.is_none(), "{}", listing.metadata);
    assert_eq!(listing.related, Vec::<(String, String)>::new());
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_lists_its_related_resources_in_the_shape_the_simulator_does() {
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let real = listing_as_promised(&address);
    let scenario = RemoteScenario::accepting(KEY);
    let remote = Remote::start(if real.related.is_empty() {
        scenario
    } else {
        scenario.relating(Related::new("notes", &["notes"]))
    });
    let simulated = listing_as_promised(&remote.address());
    let fields = [
        "resource",
        "authorization_servers",
        "scopes_supported",
        RELATED_RESOURCES,
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
    let shape = |resource: &str| present(&related_metadata(resource).body, &fields);
    let expected = simulated
        .related
        .first()
        .map(|(resource, _)| shape(resource));
    for (resource, kind) in &real.related {
        assert_eq!(Some(shape(resource)), expected, "{kind}");
    }
}

//! What a sign-in says of the companion resources it leaves out, as text that the remote gave:
//! each control character and each bidirectional control of it written escaped, and an
//! identifier outside visible ASCII left out before anything is asked of it.

use std::time::Duration;

use serde_json::json;
use software_plant::remote::{Remote, RemoteScenario};

use super::{Learned, RESOURCE_METADATA, learned, listed, vouched};
use crate::escaped::{ESCAPED, HOSTILE, shown};

/// The address of a remote's document, and the remote's own protected resource.
const ADDRESS: &str = "https://h.example/documents/one";
const OWN: &str = "https://h.example/documents";

#[test]
fn a_listing_leaves_out_an_identifier_outside_visible_ascii_and_names_each_entry_escaped() {
    let metadata = json!({ "companion_resources": [
        { "resource": "https://h.example/notes", "kind": "notes" },
        { "resource": "https://h.example/b\u{fc}cher", "kind": "books" },
        { "resource": "https://h.example/a b", "kind": "spaced" },
        { "resource": "https://h.example/in\u{9b}box\u{202e}", "kind": HOSTILE },
        { "resource": format!("https://h.example/part#{HOSTILE}"), "kind": "fragment" },
        { "resource": "https://h.example/notes", "kind": format!("again{HOSTILE}") },
        { "resource": "https://g.example/x", "kind": format!("elsewhere{HOSTILE}") },
        { "resource": HOSTILE, "kind": "bare" },
    ]});
    let (read, warnings) = listed(ADDRESS, OWN, &metadata);
    let read: Vec<(&str, &str)> = (read.iter())
        .map(|(resource, kind)| (resource.as_str(), kind.as_str()))
        .collect();
    assert_eq!(read, [("https://h.example/notes", "notes")]);
    let outside = "its identifier holds a character other than visible ASCII";
    assert_eq!(
        warnings,
        [
            format!("leaves out https://h.example/b\u{fc}cher (books): {outside}"),
            format!("leaves out https://h.example/a b (spaced): {outside}"),
            format!(
                "leaves out https://h.example/in\\u{{9b}}box\\u{{202e}} ({ESCAPED}): {outside}"
            ),
            format!(
                "leaves out https://h.example/part#{ESCAPED} (fragment): its identifier holds \
                 user information or a fragment"
            ),
            format!(
                "leaves out https://h.example/notes (again{ESCAPED}): an entry before it names \
                 the same resource or kind"
            ),
            format!(
                "leaves out https://g.example/x (elsewhere{ESCAPED}): it does not lie on the \
                 origin of the address"
            ),
            format!("leaves out {ESCAPED} (bare): {outside}"),
        ]
    );
    assert!(
        warnings.iter().all(|warning| shown(warning)),
        "{warnings:?}"
    );
}

#[test]
fn a_companion_s_metadata_that_does_not_vouch_for_it_is_named_escaped() {
    let (resource, issuer) = ("https://h.example/notes", "https://h.example/auth");
    let metadata = |named: &str, scopes: serde_json::Value| {
        json!({
            "resource": named,
            "authorization_servers": [issuer],
            "scopes_supported": scopes,
        })
    };
    let misnamed = metadata(&format!("{resource}#{HOSTILE}"), json!(["notes"]));
    let said = format!("its metadata names another resource, {resource}#{ESCAPED}");
    assert_eq!(vouched(resource, issuer, &misnamed), Err(said));
    // A string between double quotes; any other value as JSON, which escapes some control
    // characters its own way, and then escaped as a whole.
    let object = r#"{"scope":"x\\u001b]0;y\\u0007\u{9b}2J\u{202e}\\\\z"}"#;
    for (value, written) in [
        (json!(HOSTILE), format!("\"{ESCAPED}\"")),
        (json!({ "scope": HOSTILE }), object.to_owned()),
    ] {
        let unscoped = metadata(resource, json!(["notes", value]));
        let said = format!("its metadata lists {written}, which is not a scope");
        let refused = vouched(resource, issuer, &unscoped);
        assert_eq!(refused, Err(said), "{value}");
        assert!(shown(&refused.unwrap_err()));
    }
}

#[test]
fn the_address_of_a_companion_s_metadata_is_named_escaped_whether_it_answers_or_cannot_be_reached()
{
    // A path whose characters outside visible ASCII the parser of a request reads: a listing
    // leaves such an identifier out, and the words of its metadata stay escaped all the same.
    let path = "/in\u{9b}box\u{202e}";
    let at = |origin: &str| format!("{origin}{RESOURCE_METADATA}/in\\u{{9b}}box\\u{{202e}}");
    let timeout = Duration::from_secs(30);
    let remote = Remote::start(RemoteScenario::accepting("doc-hostile-metadata"));
    let origin = remote.origin();
    let Learned::Unvouched(why) = learned(&format!("{origin}{path}"), "issuer", timeout) else {
        panic!("the simulator answers the metadata");
    };
    let answered = at(origin) + " answered 404 without a metadata document";
    assert_eq!(why, answered);
    // The closed loopback port that no test serves on.
    let closed = "http://127.0.0.1:9";
    let Learned::Unreached(why) = learned(&format!("{closed}{path}"), "issuer", timeout) else {
        panic!("nothing listens on {closed}");
    };
    let unreached = format!("its metadata cannot be reached: {}: ", at(closed));
    assert!(why.starts_with(&unreached), "{why}");
    assert!(shown(&why), "{why:?}");
}

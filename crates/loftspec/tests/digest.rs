//! Domain-separated digests over RFC 8785 canonical JSON (RQ-12).

use loftspec::canonical::{Digest, Domain, canonical_bytes, digest};
use serde_json::json;

#[test]
fn the_domain_strings_are_closed() {
    let names: Vec<&str> = Domain::ALL.iter().map(|domain| domain.as_str()).collect();
    assert_eq!(
        names,
        [
            "loftspec/node/1",
            "loftspec/edges/1",
            "loftspec/registry/1",
            "loftspec/decision/1",
            "loftspec/decisions/1",
            "loftspec/content/1",
            "loftspec/bindings/1",
            "loftspec/state/1",
            "loftspec/extensions/1",
        ]
    );
}

#[test]
fn one_view_under_two_domains_yields_two_digests() {
    let view = json!([
        "feature",
        "Keys",
        null,
        [],
        null,
        null,
        null,
        null,
        null,
        []
    ]);
    let digests: std::collections::BTreeSet<Digest> = Domain::ALL
        .iter()
        .map(|domain| digest(*domain, &view))
        .collect();
    assert_eq!(digests.len(), Domain::ALL.len());
}

#[test]
fn canonical_bytes_follow_rfc_8785() {
    let view =
        json!({"b": [1, "é", null], "a": "line\nbreak\ttab \"quoted\"", "c": {"z": true, "y": 0}});
    assert_eq!(
        String::from_utf8(canonical_bytes(&view)).unwrap(),
        r#"{"a":"line\nbreak\ttab \"quoted\"","b":[1,"é",null],"c":{"y":0,"z":true}}"#
    );
}

#[test]
fn a_digest_is_sha256_over_the_domain_a_zero_byte_and_the_canonical_bytes() {
    // printf 'loftspec/node/1\0[]' | shasum -a 256
    assert_eq!(
        digest(Domain::Node, &json!([])).to_string(),
        "c8a8921e3f8b679831390547158755191a706953c7ce97b31e95fcf0db385fff"
    );
    // printf 'loftspec/edges/1\0["a",1,null]' | shasum -a 256
    assert_eq!(
        digest(Domain::Edges, &json!(["a", 1, null])).to_string(),
        "1da74a16685a42e404a0ffe01d3ed6d0515c727cb1571959cac8b9611c996ff9"
    );
}

#[test]
fn a_digest_reads_back_from_its_text() {
    let computed = digest(Domain::State, &json!([]));
    assert_eq!(computed.to_string().parse::<Digest>().unwrap(), computed);
    assert!(
        "C8A8921E3F8B679831390547158755191A706953C7CE97B31E95FCF0DB385FFF"
            .parse::<Digest>()
            .is_err()
    );
}

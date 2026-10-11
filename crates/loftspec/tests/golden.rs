//! The golden vectors published with the Format are reproduced byte for byte (AC-48). The
//! vectors in `testdata/golden/` were computed by an independent implementation; never
//! regenerate them to make this test pass.

mod common;

use common::small_document;
use loftspec::canonical::{Domain, canonical_bytes, digest};
use loftspec::hash::{hash_document, node_view};
use serde_json::Value;

const SMALL_DOCUMENT: &str = include_str!("../../../testdata/golden/small-document.json");

struct Computed {
    name: String,
    domain: Domain,
    canonical: String,
}

fn computed_views() -> Vec<Computed> {
    let document = small_document();
    let hashes = hash_document(&document).unwrap();
    let mut views = Vec::new();
    let mut push = |name: String, domain: Domain, view: &Value| {
        let canonical = String::from_utf8(canonical_bytes(view)).unwrap();
        views.push(Computed {
            name,
            domain,
            canonical,
        });
    };
    for node in document.nodes() {
        push(
            format!("node:{}", node.key.unwrap()),
            Domain::Node,
            &node_view(node),
        );
    }
    for (key, view) in &hashes.decision_views {
        push(format!("decision:{key}"), Domain::Decisions, view);
    }
    for (name, domain, view) in &hashes.root_views {
        push(format!("root:{name}"), *domain, view);
    }
    views
}

#[test]
fn ac_48_the_small_document_vectors_are_reproduced_byte_for_byte() {
    let golden: Value = serde_json::from_str(SMALL_DOCUMENT).unwrap();
    let expected = golden["views"].as_array().unwrap();
    let computed = computed_views();
    assert_eq!(computed.len(), expected.len(), "one view per golden vector");
    for view in &computed {
        let vector = expected
            .iter()
            .find(|vector| vector["name"] == view.name.as_str())
            .unwrap_or_else(|| panic!("no golden vector named {}", view.name));
        assert_eq!(vector["domain"], view.domain.as_str(), "{}", view.name);
        assert_eq!(
            vector["canonical"],
            view.canonical.as_str(),
            "{}",
            view.name
        );
        let hash = digest(
            view.domain,
            &serde_json::from_str::<Value>(&view.canonical).unwrap(),
        );
        assert_eq!(vector["digest"], hash.to_string().as_str(), "{}", view.name);
    }
}

#[test]
fn the_small_document_roots_match_their_vectors() {
    let golden: Value = serde_json::from_str(SMALL_DOCUMENT).unwrap();
    let roots = hash_document(&small_document()).unwrap().roots;
    let vector = |name: &str| {
        golden["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|vector| vector["name"] == name)
            .unwrap()["digest"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(roots.content.to_string(), vector("root:content"));
    assert_eq!(roots.bindings.to_string(), vector("root:bindings"));
    assert_eq!(roots.state.to_string(), vector("root:state"));
}

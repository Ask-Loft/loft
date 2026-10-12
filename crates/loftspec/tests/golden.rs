//! The golden vectors published with the Format are reproduced byte for byte (AC-48). The
//! vectors in `testdata/golden/` were computed by an independent implementation; never
//! regenerate them to make this test pass.

mod common;

use common::small_document;
use loftspec::canonical::{Domain, canonical_bytes, digest};
use loftspec::hash::{hash_document, hash_indexed_with_views};
use serde_json::Value;

const SMALL_DOCUMENT: &str = include_str!("../../../testdata/golden/small-document.json");

struct Computed {
    name: String,
    domain: Domain,
    canonical: String,
}

fn computed_views() -> Vec<Computed> {
    let (_, kept) = hash_indexed_with_views(&small_document().index().unwrap());
    let mut views = Vec::new();
    let mut push = |name: String, domain: Domain, view: &Value| {
        let canonical = String::from_utf8(canonical_bytes(view)).unwrap();
        views.push(Computed {
            name,
            domain,
            canonical,
        });
    };
    for (id, view) in &kept.nodes {
        push(format!("node:{}", id.key()), Domain::Node, view);
    }
    for (id, view) in &kept.decisions {
        push(format!("decision:{id}"), Domain::Decision, view);
    }
    for (name, domain, view) in &kept.roots {
        push(format!("root:{name}"), *domain, view);
    }
    views
}

fn vectors() -> Vec<Value> {
    let golden: Value = serde_json::from_str(SMALL_DOCUMENT).unwrap();
    golden["views"].as_array().unwrap().clone()
}

#[test]
fn the_small_document_vectors_are_reproduced_byte_for_byte() {
    let expected = vectors();
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
    let vectors = vectors();
    let vector = |name: &str| {
        vectors
            .iter()
            .find(|vector| vector["name"] == name)
            .unwrap()["digest"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let roots = hash_document(&small_document()).unwrap().roots;
    assert_eq!(roots.content.to_string(), vector("root:content"));
    assert_eq!(roots.bindings.to_string(), vector("root:bindings"));
    assert_eq!(roots.state.to_string(), vector("root:state"));
}

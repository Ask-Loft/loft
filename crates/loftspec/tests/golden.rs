//! The golden vectors published with the Format are reproduced byte for byte (AC-48). The
//! vectors in `testdata/golden/` were computed by an independent implementation; never
//! regenerate them to make this test pass.

use crate::common::small_document;
use loftspec::canonical::{Domain, canonical_bytes, digest};
use loftspec::codec::{self, SpecPath};
use loftspec::hash::{
    StateView, hash_document, hash_indexed_with_views, lint_digest, state_root, state_view,
};
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

const STATE_VIEW_2: &str =
    include_str!("../../../testdata/golden/small-document-state-view-2.json");

#[test]
fn the_small_documents_state_view_2_vectors_are_reproduced_byte_for_byte() {
    let golden: Value = serde_json::from_str(STATE_VIEW_2).unwrap();
    let lint = golden["lintConfiguration"].as_str().unwrap();
    let document = small_document();
    let roots = hash_document(&document).unwrap().roots;
    let view = state_view(&document, &roots, StateView::Two, Some(lint));
    let views = golden["views"].as_array().unwrap();
    let canonical = String::from_utf8(canonical_bytes(&view)).unwrap();
    assert_eq!(views[1]["canonical"], canonical.as_str());
    assert_eq!(views[0]["digest"], lint_digest(lint).to_string().as_str());
    let state = state_root(&document, &roots, StateView::Two, Some(lint));
    assert_eq!(views[1]["digest"], state.to_string().as_str());
    assert_eq!(
        views[0]["canonical"],
        String::from_utf8(canonical_bytes(&lint)).unwrap().as_str()
    );
}

const CONFLICT_IDENTIFIERS: &str =
    include_str!("../../../testdata/golden/conflict-identifiers.json");

#[test]
fn the_conflict_identifier_vectors_are_reproduced_byte_for_byte() {
    let golden: Value = serde_json::from_str(CONFLICT_IDENTIFIERS).unwrap();
    assert_eq!(golden["domain"], Domain::Conflict.as_str());
    for vector in golden["conflicts"].as_array().unwrap() {
        let name = &vector["name"];
        let path = SpecPath::new(vector["path"].as_str().unwrap());
        let block = vector["yaml"].as_str().unwrap();
        let mut files = codec::write(&small_document());
        let text = String::from_utf8(files[&path].clone()).unwrap();
        let text = match vector["after"].as_str() {
            Some(after) => text.replacen(after, &format!("{after}{block}"), 1),
            None => format!("{text}{block}"),
        };
        files.insert(path, text.into_bytes());
        let read = codec::read(&files).unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
        let [conflict] = read.conflicts.as_slice() else {
            panic!("{name}: one conflict in {:?}", read.conflicts);
        };
        let canonical = String::from_utf8(canonical_bytes(&conflict.view())).unwrap();
        assert_eq!(vector["canonical"], canonical.as_str(), "{name}");
        assert_eq!(
            vector["digest"],
            conflict.id().to_string().as_str(),
            "{name}"
        );
    }
}

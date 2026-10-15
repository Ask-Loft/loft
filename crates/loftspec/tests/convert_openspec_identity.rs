//! The `OpenSpec` converter's keys, its determinism and its provenance sidecar: keys derived from
//! each node's place in the source, identical output for identical input, no labels, and a
//! sidecar that records each node's source outside the tree.

use std::fmt::Write as _;

use crate::common::openspec::{AUTH, DOCUMENT, converted, document, features, one, project};
use loftspec::bindings::{Bindings, Counters};
use loftspec::codec;
use loftspec::convert::openspec::{Provenance, SIDECAR, convert};
use loftspec::identity::Key;
use loftspec::ids::DocumentId;
use sha2::{Digest, Sha256};
use uuid::Builder;

/// The key the derivation gives a node of `kind` with the source identity `parts`, computed
/// here from the rule as the specification states it.
fn expected(kind: &str, parts: &[&str]) -> Key {
    let mut digest = Sha256::new();
    for part in ["loft-openspec-key/1", DOCUMENT, kind].iter().chain(parts) {
        digest.update(part.as_bytes());
        digest.update([0]);
    }
    let bytes: [u8; 16] = digest.finalize()[..16].try_into().unwrap();
    Key::from(Builder::from_custom_bytes(bytes).into_uuid())
}

fn keys_of(nodes: &[Provenance], kind: &str, name: &str) -> Vec<Key> {
    nodes
        .iter()
        .filter(|node| node.kind == kind && node.name.as_deref() == Some(name))
        .map(|node| node.key)
        .collect()
}

const NOTES: &str = "## Purpose
Notes.

## Design
Text.

## Requirements

### Requirement: Sign in
The system SHALL let a user sign in.

#### Scenario: Valid credentials
- ok

#### Rationale
Why.

### Requirement: Sign in
The system SHALL let an administrator sign in.

#### Scenario: Valid credentials
- ok
";

#[test]
fn each_nodes_key_derives_from_the_document_key_its_kind_and_its_source_identity() {
    let conversion = converted(&project(&[
        ("auth/spec.md", NOTES),
        ("billing/invoices/spec.md", AUTH),
    ]));
    let nodes = &conversion.sidecar.nodes;
    let product = &conversion.document.products[0];
    assert_eq!(product.id.unwrap().key(), expected("product", &[]));
    let groups: Vec<Key> = product
        .feature_groups
        .iter()
        .map(|group| group.id.unwrap().key())
        .collect();
    assert_eq!(
        groups,
        [
            expected("feature-group", &[""]),
            expected("feature-group", &["billing"])
        ]
    );
    let feature = features(&conversion)[0];
    assert_eq!(feature.id.unwrap().key(), expected("feature", &["auth"]));
    assert_eq!(
        keys_of(nodes, "requirement", "Sign in")[0],
        expected("requirement", &["auth", "Sign in"])
    );
    assert_eq!(
        keys_of(nodes, "criterion", "Valid credentials")[0],
        expected("criterion", &["auth", "Sign in", "Valid credentials"])
    );
    assert_eq!(
        keys_of(nodes, "section", "Design"),
        [expected("section", &["feature", "auth", "Design"])]
    );
    assert_eq!(
        keys_of(nodes, "section", "Rationale"),
        [expected(
            "section",
            &["requirement", "auth", "Sign in", "Rationale"]
        )]
    );
    assert!(nodes.iter().all(|node| node.key.version() == 8));
}

#[test]
fn a_repeated_identity_adds_its_ordinal_and_its_children_start_from_it() {
    let conversion = converted(&one(NOTES));
    let nodes = &conversion.sidecar.nodes;
    assert_eq!(
        keys_of(nodes, "requirement", "Sign in"),
        [
            expected("requirement", &["auth", "Sign in"]),
            expected("requirement", &["auth", "Sign in", "2"])
        ]
    );
    assert_eq!(
        keys_of(nodes, "criterion", "Valid credentials")[1],
        expected("criterion", &["auth", "Sign in", "2", "Valid credentials"])
    );
}

#[test]
fn another_document_key_gives_other_keys() {
    let other = DocumentId::new(Key::parse("01a13d20-0000-7000-8000-000000000002").unwrap());
    let first = converted(&one(AUTH));
    let second = convert(&one(AUTH), other, "Shop").unwrap();
    assert_ne!(
        features(&first)[0].id,
        second.document.products[0].feature_groups[0].features[0].id
    );
}

#[test]
fn identical_input_and_document_key_give_byte_identical_trees_and_sidecars() {
    let project = project(&[("auth/spec.md", NOTES), ("billing/invoices/spec.md", AUTH)]);
    let first = convert(&project, document(), "Shop").unwrap();
    let second = convert(&project, document(), "Shop").unwrap();
    assert_eq!(
        codec::write(&first.document),
        codec::write(&second.document)
    );
    assert_eq!(first.sidecar.to_json(), second.sidecar.to_json());
}

#[test]
fn the_tree_binds_no_label_and_its_counters_hold_their_starting_values() {
    let document = converted(&one(AUTH)).document;
    assert_eq!(document.bindings, Bindings::default());
    assert_eq!(document.counters, Counters::default());
}

#[test]
fn the_tree_reads_back_strictly_and_every_node_has_a_key() {
    let conversion = converted(&project(&[
        ("auth/spec.md", NOTES),
        ("billing/invoices/spec.md", AUTH),
    ]));
    let files = codec::write(&conversion.document);
    let read = codec::read(&files).expect("the converted tree reads strictly");
    assert_eq!(read, conversion.document);
}

#[test]
fn the_sidecar_records_each_nodes_source() {
    let conversion = converted(&one(NOTES));
    let nodes = &conversion.sidecar.nodes;
    let feature = nodes.iter().find(|node| node.kind == "feature").unwrap();
    assert_eq!(feature.capability.as_deref(), Some("auth"));
    assert_eq!(feature.path.as_deref(), Some("specs/auth/spec.md"));
    let at = |kind: &str, name: &str| {
        let node = nodes
            .iter()
            .find(|node| node.kind == kind && node.name.as_deref() == Some(name))
            .unwrap();
        (node.path.as_deref(), node.line)
    };
    let file = Some("specs/auth/spec.md");
    assert_eq!(at("requirement", "Sign in"), (file, Some(9)));
    assert_eq!(at("criterion", "Valid credentials"), (file, Some(12)));
    assert_eq!(at("section", "Design"), (file, Some(4)));
    assert_eq!(at("section", "Rationale"), (file, Some(15)));
    let [source] = conversion.sidecar.sources.as_slice() else {
        panic!("one source");
    };
    assert_eq!(source.path, "specs/auth/spec.md");
    let digest = Sha256::digest(NOTES.as_bytes())
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    assert_eq!(source.sha256, digest);
}

#[test]
fn the_sidecar_lies_beside_spec_and_no_file_of_the_tree_records_a_source() {
    assert_eq!(SIDECAR, "openspec-sidecar.json");
    let conversion = converted(&one(NOTES));
    for (path, bytes) in codec::write(&conversion.document) {
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("specs/"), "{path}: {text}");
        assert!(!text.contains("line:"), "{path}: {text}");
    }
}

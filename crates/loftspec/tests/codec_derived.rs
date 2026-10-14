//! The writer writes no derived value: no node record carries a hash, an ordinal or a label,
//! and no file under `spec/` carries a digest the hasher computes. Each check reads the full
//! document's written tree, in which every optional field of every record is filled.

use crate::common::full::{full_document, full_files};
use crate::common::text::text;
use loftspec::canonical::Digest;
use loftspec::codec::Files;
use loftspec::hash::hash_document;
use loftspec::yaml::{self, Node, Value};

/// Every field name and every scalar under `node`.
fn walk(node: &Node, names: &mut Vec<String>, scalars: &mut Vec<String>) {
    match &node.value {
        Value::Mapping(entries) => {
            for entry in entries {
                names.push(entry.key.clone());
                walk(&entry.value, names, scalars);
            }
        }
        Value::Sequence(items) => {
            for item in items {
                walk(item, names, scalars);
            }
        }
        Value::Scalar(scalar) => scalars.push(scalar.text.clone()),
        Value::Empty => {}
    }
}

/// The field names and the scalars of every file that holds node records: the Product, Feature
/// and Component files.
fn node_records(files: &Files) -> (Vec<String>, Vec<String>) {
    let (mut names, mut scalars) = (Vec::new(), Vec::new());
    let node_files = files.iter().filter(|(at, _)| {
        at.as_str().starts_with("products/") || at.as_str().starts_with("components/")
    });
    for (_, bytes) in node_files {
        walk(
            &yaml::read(bytes).unwrap().unwrap(),
            &mut names,
            &mut scalars,
        );
    }
    assert!(names.len() > 100, "the node files hold every field");
    (names, scalars)
}

/// Every digest the hasher computes for the full document.
fn digests() -> Vec<Digest> {
    let hashes = hash_document(&full_document()).unwrap();
    let roots = hashes.roots;
    let mut digests = vec![
        roots.edges,
        roots.tags,
        roots.categories,
        roots.decisions,
        roots.content,
        roots.bindings,
        roots.state,
    ];
    digests.extend(hashes.nodes.values());
    digests.extend(hashes.decisions.values());
    digests.extend(hashes.products);
    digests.extend(hashes.components);
    digests
}

#[test]
fn no_written_node_record_carries_a_hash() {
    let (names, scalars) = node_records(&full_files());
    let field = names
        .iter()
        .find(|name| name.to_lowercase().contains("hash"));
    assert_eq!(field, None);
    let nodes = hash_document(&full_document()).unwrap().nodes;
    for digest in nodes.values() {
        assert!(!scalars.contains(&digest.to_string()), "{digest}");
    }
}

#[test]
fn no_written_node_record_carries_an_ordinal() {
    let (names, _) = node_records(&full_files());
    let ordinal = names.iter().find(|name| {
        let name = name.to_lowercase();
        ["ordinal", "position", "index"]
            .iter()
            .any(|word| name.contains(word))
    });
    assert_eq!(ordinal, None);
}

#[test]
fn no_written_node_record_carries_a_label() {
    let (names, scalars) = node_records(&full_files());
    let labelled = names.iter().find(|name| {
        let name = name.to_lowercase();
        name.contains("label") || name.contains("alias")
    });
    assert_eq!(labelled, None);
    let document = full_document();
    for binding in document.bindings.iter() {
        for label in binding.every_label() {
            assert!(!scalars.contains(&label.to_string()), "{label}");
        }
    }
}

#[test]
fn no_written_file_carries_a_digest_the_hasher_computes() {
    let files = full_files();
    let digests = digests();
    assert!(digests.len() > 20, "a digest for every node and root");
    for at in files.keys() {
        let written = text(&files, at.as_str());
        for digest in &digests {
            assert!(
                !written.contains(&digest.to_string()),
                "{at} holds {digest}"
            );
        }
    }
}

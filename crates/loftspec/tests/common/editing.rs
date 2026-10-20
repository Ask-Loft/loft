//! The document the editing tests change, and what they read of it.

use loftspec::diff::{NodeChanges, node_changes};
use loftspec::document::Document;
use loftspec::edit::{self, Found, Parent};
use loftspec::ids::{ComponentId, FeatureGroupId, FeatureId, NodeId};
use loftspec::node::Node;
use loftspec::tree::{Details, FeatureGroup};

use super::full::full_nested_document;
use super::{criterion, feature, key, requirement, slug};

pub const PRODUCT: u32 = 1;
pub const GROUP: u32 = 2;
pub const KEYS: u32 = 3;
pub const LABELS: u32 = 100;
pub const SLUGS: u32 = 101;
pub const STORAGE: u32 = 102;
/// The root Component, its nested Component and the Component nested in that one.
pub const ROOT: u32 = 6;
pub const NESTED: u32 = 61;
pub const INNER: u32 = 66;

/// The full document with its nested Component filled, two more Features in its Feature Group
/// after Keys (Labels, keyed 100, with a Requirement keyed 103 and its criterion 104; and Slugs,
/// keyed 101), and a second Feature Group, Storage (keyed 102), that holds no Feature.
pub fn editable() -> Document {
    let mut document = full_nested_document();
    let criteria = vec![criterion(key(104), "Each label is allocated once.")];
    let labels = feature(
        key(LABELS),
        "Labels",
        vec![requirement(key(103), "Labels are allocated.", criteria)],
    );
    let product = &mut document.products[0];
    product.feature_groups[0].features.push(labels);
    product.feature_groups[0]
        .features
        .push(feature(key(SLUGS), "Slugs", Vec::new()));
    product.feature_groups.push(FeatureGroup {
        id: Some(FeatureGroupId::new(key(STORAGE))),
        slug: slug("storage"),
        name: "Storage".to_owned(),
        details: Details::default(),
        features: Vec::new(),
        constraints: Vec::new(),
        quality_attributes: Vec::new(),
    });
    document
}

pub fn feature_id(n: u32) -> NodeId {
    FeatureId::new(key(n)).into()
}

/// The number of each Feature of the Feature Group at `group`, in order.
pub fn features(document: &Document, group: usize) -> Vec<u32> {
    document.products[0].feature_groups[group]
        .features
        .iter()
        .map(|feature| number(feature.id.unwrap().key()))
        .collect()
}

/// The number `n` whose key is `key`.
pub fn number(key: loftspec::identity::Key) -> u32 {
    (0..0x200)
        .find(|n| super::key(*n) == key)
        .expect("a small key")
}

/// The number of every keyed node at and beneath `node`, parents first.
pub fn beneath(node: Node<'_>) -> Vec<u32> {
    let mut numbers = Vec::new();
    node.walk(&mut |each| numbers.extend(each.id().map(|id| number(id.key()))));
    numbers
}

/// What `after` added, changed, moved and removed against `before`, by number.
pub struct Changes {
    pub added: Vec<u32>,
    pub changed: Vec<u32>,
    pub moved: Vec<u32>,
    pub removed: Vec<u32>,
}

pub fn changes(before: &Document, after: &Document) -> Changes {
    let NodeChanges {
        added,
        changed,
        removed,
        moved,
    } = node_changes(before, after);
    let numbers = |ids: Vec<NodeId>| {
        let mut numbers: Vec<u32> = ids.into_iter().map(|id| number(id.key())).collect();
        numbers.sort_unstable();
        numbers
    };
    Changes {
        added: numbers(added),
        changed: numbers(changed),
        moved: numbers(moved),
        removed: numbers(removed),
    }
}

/// The Feature Group keyed `n`, as a parent.
pub fn group(n: u32) -> Parent {
    Parent::Node(FeatureGroupId::new(key(n)).into())
}

/// The Component keyed `n`.
pub fn component(n: u32) -> NodeId {
    ComponentId::new(key(n)).into()
}

/// The node `n` finds, which must be a node.
pub fn found_node(document: &Document, n: u32) -> Node<'_> {
    match edit::find(document, key(n)) {
        Some(Found::Node(node)) => node,
        other => panic!("key {n} found {other:?}"),
    }
}

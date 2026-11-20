//! A copy of another document's Interface: the Interface, with everything
//! beneath it and the labels its own document bound, as that document published it at one
//! publish sequence. The copy is the pin: references into the other document resolve against it
//! and the hasher reads the foreign nodes' content from it. Its nodes are kept apart from the
//! document's own, so nothing labels, lints or formats them.

use std::collections::{BTreeMap, BTreeSet};

use crate::canonical::Digest;
use crate::codec::{Role, SpecPath};
use crate::document::{Edge, EdgeKind, EdgeTarget};
use crate::identity::{Key, Label};
use crate::ids::{ElementId, NodeId};
use crate::node::Node;
use crate::slug::Slug;
use crate::tree::Interface;

/// One copy, under the handle of the other document's entry of `documents`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copy {
    pub handle: Slug,
    /// The publish sequence of the other document's state the copy was taken from.
    pub publish_sequence: u64,
    /// That state's state root.
    pub state_root: Digest,
    /// The Interface's content hash in that state.
    pub interface_hash: Digest,
    /// The label of the Interface and of each node beneath it that has one, by key.
    pub labels: BTreeMap<Key, Label>,
    /// The Interface, read without its edges, which name the other document's own elements.
    pub interface: Interface,
    /// The copy's file, exactly as `loft interface` wrote it.
    pub bytes: Vec<u8>,
}

impl Copy {
    /// The copy's file: `spec/documents/<handle>/<slug>.yaml`.
    #[must_use]
    pub fn path(&self) -> SpecPath {
        self.role().path()
    }

    /// The role of the copy's file.
    #[must_use]
    pub fn role(&self) -> Role {
        Role::Copy {
            handle: self.handle.as_str().to_owned(),
            slug: self.interface.slug.as_str().to_owned(),
        }
    }

    /// Every node of the copy, the Interface first, in document order.
    #[must_use]
    pub fn ids(&self) -> Vec<NodeId> {
        let mut ids = Vec::new();
        Node::Interface(&self.interface).walk(&mut |node| ids.extend(node.id()));
        ids
    }

    /// The key of every node of the copy, the Interface's first, in document order.
    #[must_use]
    pub fn keys(&self) -> Vec<Key> {
        self.ids().into_iter().map(NodeId::key).collect()
    }

    /// The copied Interface's key.
    ///
    /// # Panics
    ///
    /// The Interface carries no key, which a copy `loft interface` writes always does.
    #[must_use]
    pub fn key(&self) -> Key {
        self.interface
            .id
            .expect("a copied Interface carries its key")
            .key()
    }

    /// The node `key` of the copy as `<handle>:<label>`, or `<handle>:<key>` where the other
    /// document bound it no label.
    #[must_use]
    pub fn name_of(&self, key: Key) -> String {
        match self.labels.get(&key) {
            Some(label) => format!("{}:{label}", self.handle),
            None => format!("{}:{key}", self.handle),
        }
    }

    /// The first way this copy differs from `served`, the copy of the same Interface taken from
    /// the state its pin names: its state root, its Interface, the label of a node, then its
    /// bytes; none where it is that copy.
    #[must_use]
    pub fn difference(&self, served: &Self) -> Option<CopyDifference> {
        if self.state_root != served.state_root {
            return Some(CopyDifference::StateRoot {
                held: self.state_root,
                served: served.state_root,
            });
        }
        if self.interface_hash != served.interface_hash {
            return Some(CopyDifference::Interface);
        }
        let keys = self.labels.keys().chain(served.labels.keys());
        let labeled = keys
            .copied()
            .find(|key| self.labels.get(key) != served.labels.get(key));
        if let Some(key) = labeled {
            return Some(CopyDifference::Label {
                key,
                held: self.labels.get(&key).copied(),
                served: served.labels.get(&key).copied(),
            });
        }
        (self.bytes != served.bytes).then_some(CopyDifference::Bytes)
    }
}

/// How a copy differs from the copy its pinned state gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyDifference {
    /// The copy names another state root than the state's.
    StateRoot { held: Digest, served: Digest },
    /// The copy's Interface is not the one the state holds.
    Interface,
    /// The copy binds the node `key` another label than the state does, or none.
    Label {
        key: Key,
        held: Option<Label>,
        served: Option<Label>,
    },
    /// The copy says the same in other bytes than `loft interface` writes.
    Bytes,
}

/// Two copies that cannot stand in one tree, and what they share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash<'a> {
    pub first: &'a Copy,
    pub second: &'a Copy,
    pub shared: Shared,
}

/// What two clashing copies share: their file, or a node's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shared {
    File(SpecPath),
    Key(Key),
}

/// The first two of `copies` that share a file or a node's key, which no tree may hold.
#[must_use]
pub fn clash<'a>(copies: &[&'a Copy]) -> Option<Clash<'a>> {
    let mut files: BTreeMap<SpecPath, &Copy> = BTreeMap::new();
    let mut keys: BTreeMap<Key, &Copy> = BTreeMap::new();
    for &copy in copies {
        let path = copy.path();
        if let Some(&first) = files.get(&path) {
            return Some(Clash {
                first,
                second: copy,
                shared: Shared::File(path),
            });
        }
        files.insert(path, copy);
        for key in copy.keys() {
            if let Some(&first) = keys.get(&key) {
                return Some(Clash {
                    first,
                    second: copy,
                    shared: Shared::Key(key),
                });
            }
            keys.insert(key, copy);
        }
    }
    None
}

/// The key of the node that the copies under `handle` bind to `label`.
#[must_use]
pub fn labeled(copies: &[Copy], handle: &str, label: Label) -> Option<Key> {
    (copies.iter())
        .filter(|copy| copy.handle.as_str() == handle)
        .flat_map(|copy| copy.labels.iter())
        .find(|(_, bound)| **bound == label)
        .map(|(key, _)| *key)
}

/// A reference of a document into a copy: the element that makes it, the kind of its edge and
/// the key of the copy's node it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    pub source: ElementId,
    pub kind: EdgeKind,
    pub key: Key,
}

/// Each edge of `edges` whose target is keyed in `keys`, in the edges' order.
#[must_use]
pub fn references(edges: &[Edge], keys: &BTreeSet<Key>) -> Vec<Reference> {
    (edges.iter())
        .filter_map(|edge| match edge.target() {
            EdgeTarget::Element(target) if keys.contains(&target.key()) => Some(Reference {
                source: edge.source(),
                kind: edge.kind(),
                key: target.key(),
            }),
            _ => None,
        })
        .collect()
}

/// The nodes of a document's copies, by key: where a reference into another document points.
#[derive(Debug, Clone, Default)]
pub struct Foreign<'a> {
    nodes: BTreeMap<Key, (&'a Copy, NodeId)>,
}

impl<'a> Foreign<'a> {
    /// The nodes of `copies`; a key that two copies carry is the first's, which the reader
    /// refuses.
    #[must_use]
    pub fn of(copies: &'a [Copy]) -> Self {
        let mut nodes = BTreeMap::new();
        for copy in copies {
            for id in copy.ids() {
                nodes.entry(id.key()).or_insert((copy, id));
            }
        }
        Self { nodes }
    }

    /// The node of a copy that `key` names.
    #[must_use]
    pub fn node(&self, key: Key) -> Option<NodeId> {
        self.nodes.get(&key).map(|(_, id)| *id)
    }

    /// The copy that holds the node `key` names.
    #[must_use]
    pub fn copy(&self, key: Key) -> Option<&'a Copy> {
        self.nodes.get(&key).map(|(copy, _)| *copy)
    }

    /// Whether `element` is a node of a copy, of the same kind.
    #[must_use]
    pub fn holds(&self, element: ElementId) -> bool {
        matches!(element, ElementId::Node(node) if self.node(node.key()) == Some(node))
    }

    /// The label the other document gave the node `key` names.
    #[must_use]
    pub fn label(&self, key: Key) -> Option<Label> {
        self.copy(key)
            .and_then(|copy| copy.labels.get(&key).copied())
    }
}

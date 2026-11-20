//! A copy of another document's Interface: the Interface, with everything
//! beneath it and the labels its own document bound, as that document published it at one
//! publish sequence. The copy is the pin: references into the other document resolve against it
//! and the hasher reads the foreign nodes' content from it. Its nodes are kept apart from the
//! document's own, so nothing labels, lints or formats them.

use std::collections::BTreeMap;

use crate::canonical::Digest;
use crate::codec::{Role, SpecPath};
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

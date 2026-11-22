//! The edges each node's record writes, as the review packet gives them beside the node: the
//! elements it mentions, the Components a Feature realizes and the Products a Component is
//! scoped to (The review packet). Each list is in the order of its keys and names each target
//! once, as the diff counts them.

use std::collections::BTreeMap;

use crate::document::{Document, Edge};
use crate::identity::Key;
use crate::ids::{ComponentId, ElementId, FeatureId, NodeId, ProductId};

/// The edges each node of one document writes.
#[derive(Default)]
pub(super) struct Written {
    mentions: BTreeMap<NodeId, Vec<ElementId>>,
    realizes: BTreeMap<FeatureId, Vec<ComponentId>>,
    scoped_to: BTreeMap<ComponentId, Vec<ProductId>>,
}

impl Written {
    pub fn of(document: &Document) -> Self {
        let mut written = Self::default();
        for edge in &document.edges {
            match *edge {
                Edge::Mentions {
                    source: ElementId::Node(node),
                    target,
                } => written.mentions.entry(node).or_default().push(target),
                Edge::Realizes { feature, component } => {
                    written.realizes.entry(feature).or_default().push(component);
                }
                Edge::ScopedTo { component, product } => {
                    written
                        .scoped_to
                        .entry(component)
                        .or_default()
                        .push(product);
                }
                Edge::Mentions { .. }
                | Edge::Subject { .. }
                | Edge::Provides { .. }
                | Edge::Requires { .. } => {}
            }
        }
        written
            .mentions
            .values_mut()
            .for_each(|targets| in_key_order(targets, |id| id.key()));
        written
            .realizes
            .values_mut()
            .for_each(|targets| in_key_order(targets, |id| id.key()));
        written
            .scoped_to
            .values_mut()
            .for_each(|targets| in_key_order(targets, |id| id.key()));
        written
    }

    /// The elements the node `id` mentions.
    pub fn mentions(&self, id: NodeId) -> Vec<ElementId> {
        self.mentions.get(&id).cloned().unwrap_or_default()
    }

    /// The Components the node `id` realizes, where it is a Feature.
    pub fn realizes(&self, id: NodeId) -> Vec<ComponentId> {
        match id {
            NodeId::Feature(feature) => self.realizes.get(&feature).cloned().unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The Products the node `id` is scoped to, where it is a Component.
    pub fn scoped_to(&self, id: NodeId) -> Vec<ProductId> {
        match id {
            NodeId::Component(component) => {
                self.scoped_to.get(&component).cloned().unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }
}

/// Puts `targets` in the order of their keys, each once.
fn in_key_order<T>(targets: &mut Vec<T>, key: impl Fn(&T) -> Key) {
    targets.sort_unstable_by_key(&key);
    targets.dedup_by_key(|target| key(target));
}

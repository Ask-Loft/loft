//! Node content views and content hashes (RQ-11, RQ-159, RQ-160) over the typed tree.

use std::borrow::Cow;
use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::HashError;
use crate::attributes::{
    ConservationKind, QaKind, QualityTarget, TechnicalBinding, VerificationFate,
};
use crate::canonical::{Digest, Domain, digest};
use crate::ids::NodeId;
use crate::names::TagName;
use crate::node::Node;
use crate::tree::{
    Component, Constraint, Criterion, Details, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, RootComponent, Section,
};

/// Called once per node, children first, with the node's ID, content view and content hash.
pub(crate) type Visit<'a> = dyn FnMut(Option<NodeId>, Value, Digest) -> Result<(), HashError> + 'a;

/// The fields of a node's content view other than its children.
struct Fields<'a> {
    text: Cow<'a, str>,
    body: Option<&'a str>,
    tags: &'a [TagName],
    qa_kind: Option<QaKind>,
    quality_target: Option<&'a QualityTarget>,
    binding: Option<&'a TechnicalBinding>,
    conservation_kind: Option<ConservationKind>,
    verification_fate: Option<VerificationFate>,
}

impl<'a> Fields<'a> {
    /// The fields of a node whose view holds no typed attribute.
    const fn bare(text: Cow<'a, str>, body: Option<&'a str>, tags: &'a [TagName]) -> Self {
        Self {
            text,
            body,
            tags,
            qa_kind: None,
            quality_target: None,
            binding: None,
            conservation_kind: None,
            verification_fate: None,
        }
    }

    fn plain(text: impl Into<Cow<'a, str>>, details: &'a Details) -> Self {
        Self::bare(text.into(), details.body.as_deref(), &details.tags)
    }

    fn of(node: Node<'a>) -> Self {
        match node {
            Node::Product(node) => Self::plain(&node.name, &node.details),
            Node::FeatureGroup(node) => Self::plain(&node.name, &node.details),
            Node::Feature(node) => Self::plain(&node.name, &node.details),
            // A statement enters as its rendered sentence (AC-598).
            Node::Requirement(node) => Self::plain(node.statement.render(), &node.details),
            Node::Criterion(node) => Self::plain(&node.name, &node.details),
            Node::Component(node) => Self::plain(&node.name, &node.details),
            Node::Constraint(node) => Self {
                binding: node.binding.as_ref(),
                conservation_kind: node.conservation_kind,
                verification_fate: node.verification_fate,
                ..Self::plain(&node.name, &node.details)
            },
            Node::QualityAttribute(node) => Self {
                qa_kind: Some(node.kind),
                quality_target: Some(&node.target),
                verification_fate: node.verification_fate,
                ..Self::plain(&node.name, &node.details)
            },
            // A Section's view has no tags.
            Node::Section(node) => Self::bare(Cow::Borrowed(&node.name), Some(&node.body), &[]),
        }
    }
}

/// A node's content view and content hash.
pub trait ContentHash {
    /// The node, borrowed.
    fn node(&self) -> Node<'_>;

    /// The view: kind, statement or name, body, sorted tag names, the five typed attributes,
    /// then the ordered child content hashes. It is a JSON array so that RFC 8785, which sorts
    /// object members, keeps this order.
    fn content_view(&self) -> Value {
        let node = self.node();
        let children = child_hashes(node, &mut |_, _, _| Ok(()))
            .expect("hashing without an index never fails");
        view(node, &children)
    }

    /// The digest of the content view under `loftspec/node/1`.
    fn content_hash(&self) -> Digest {
        digest(Domain::Node, &self.content_view())
    }
}

/// A root Component hashes as its Component; its slug is not content.
impl ContentHash for RootComponent {
    fn node(&self) -> Node<'_> {
        Node::Component(&self.component)
    }
}

impl ContentHash for Node<'_> {
    fn node(&self) -> Node<'_> {
        *self
    }
}

macro_rules! content_hash {
    ($($kind:ident),*) => {$(
        impl ContentHash for $kind {
            fn node(&self) -> Node<'_> {
                Node::$kind(self)
            }
        }
    )*};
}

content_hash!(
    Product,
    FeatureGroup,
    Feature,
    Requirement,
    Criterion,
    Component,
    Constraint,
    QualityAttribute,
    Section
);

fn view(node: Node<'_>, children: &[Digest]) -> Value {
    let fields = Fields::of(node);
    let tags: BTreeSet<&str> = fields.tags.iter().map(TagName::as_str).collect();
    json!([
        node.kind().as_str(),
        fields.text,
        fields.body,
        tags,
        fields.qa_kind,
        fields.quality_target,
        fields.binding,
        fields.conservation_kind,
        fields.verification_fate,
        children,
    ])
}

/// The content hashes of a node's children, in canonical order (RQ-160).
fn child_hashes(node: Node<'_>, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
    node.children()
        .into_iter()
        .map(|child| hash_with(child, visit))
        .collect()
}

/// Hashes a node and everything beneath it, children first, reporting each node to `visit`.
pub(crate) fn hash_with(node: Node<'_>, visit: &mut Visit<'_>) -> Result<Digest, HashError> {
    let children = child_hashes(node, visit)?;
    let view = view(node, &children);
    let hash = digest(Domain::Node, &view);
    visit(node.id(), view, hash)?;
    Ok(hash)
}

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
use crate::kind::NodeKind;
use crate::names::TagName;
use crate::tree::{
    Component, Constraint, Criterion, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, Section,
};

/// Called once per node, children first, with the node's ID, content view and content hash.
pub(crate) type Visit<'a> = dyn FnMut(Option<NodeId>, Value, Digest) -> Result<(), HashError> + 'a;

mod sealed {
    use super::{
        ConservationKind, Cow, Digest, HashError, NodeId, NodeKind, QaKind, QualityTarget, TagName,
        TechnicalBinding, VerificationFate, Visit,
    };

    /// The fields of a node's content view other than its children.
    pub struct Fields<'a> {
        pub kind: NodeKind,
        pub text: Cow<'a, str>,
        pub body: Option<&'a str>,
        pub tags: &'a [TagName],
        pub qa_kind: Option<QaKind>,
        pub quality_target: Option<&'a QualityTarget>,
        pub binding: Option<&'a TechnicalBinding>,
        pub conservation_kind: Option<ConservationKind>,
        pub verification_fate: Option<VerificationFate>,
    }

    pub trait Parts {
        fn id(&self) -> Option<NodeId>;
        fn fields(&self) -> Fields<'_>;
        fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError>;
    }
}

use sealed::{Fields, Parts};

/// A node's content view and content hash.
pub trait ContentHash: Parts {
    /// The view: kind, statement or name, body, sorted tag names, the five typed attributes,
    /// then the ordered child content hashes. It is a JSON array so that RFC 8785, which sorts
    /// object members, keeps this order.
    fn content_view(&self) -> Value {
        let children = self
            .child_hashes(&mut |_, _, _| Ok(()))
            .expect("hashing without an index never fails");
        view(&self.fields(), &children)
    }

    /// The digest of the content view under `loftspec/node/1`.
    fn content_hash(&self) -> Digest {
        digest(Domain::Node, &self.content_view())
    }
}

impl<T: Parts> ContentHash for T {}

fn view(fields: &Fields<'_>, children: &[Digest]) -> Value {
    let tags: BTreeSet<&str> = fields.tags.iter().map(TagName::as_str).collect();
    json!([
        fields.kind.as_str(),
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

/// Hashes a node and everything beneath it, children first, reporting each node to `visit`.
pub(crate) fn hash_with(node: &impl Parts, visit: &mut Visit<'_>) -> Result<Digest, HashError> {
    let children = node.child_hashes(visit)?;
    let view = view(&node.fields(), &children);
    let hash = digest(Domain::Node, &view);
    visit(node.id(), view, hash)?;
    Ok(hash)
}

/// The fields of a node whose view holds no typed attribute.
fn plain<'a>(
    kind: NodeKind,
    text: impl Into<Cow<'a, str>>,
    body: Option<&'a str>,
    tags: &'a [TagName],
) -> Fields<'a> {
    Fields {
        kind,
        text: text.into(),
        body,
        tags,
        qa_kind: None,
        quality_target: None,
        binding: None,
        conservation_kind: None,
        verification_fate: None,
    }
}

/// Hashes one list of children in order, appending their content hashes to `hashes`.
fn extend<'a, C: Parts + 'a>(
    hashes: &mut Vec<Digest>,
    children: impl IntoIterator<Item = &'a C>,
    visit: &mut Visit<'_>,
) -> Result<(), HashError> {
    for child in children {
        hashes.push(hash_with(child, visit)?);
    }
    Ok(())
}

/// A node's child content hashes: each list of structural children in turn, then the Sections
/// (AC-605).
macro_rules! grouped {
    ($visit:expr, $sections:expr $(, $children:expr)*) => {{
        let mut hashes = Vec::new();
        $(extend(&mut hashes, $children, $visit)?;)*
        extend(&mut hashes, $sections, $visit)?;
        Ok(hashes)
    }};
}

/// `Parts` for kinds whose view holds only a name, a body and tags, with their child lists in
/// canonical order.
macro_rules! named_parts {
    ($($node:ident [$($children:ident),*];)*) => {$(
        impl Parts for $node {
            fn id(&self) -> Option<NodeId> {
                self.id.map(NodeId::from)
            }
            fn fields(&self) -> Fields<'_> {
                let details = &self.details;
                plain(NodeKind::$node, &self.name, details.body.as_deref(), &details.tags)
            }
            fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
                grouped!(visit, &self.details.sections $(, &self.$children)*)
            }
        }
    )*};
}

named_parts! {
    Product [feature_groups];
    FeatureGroup [features, constraints];
    Feature [requirements, quality_attributes];
    Criterion [];
    Component [components, constraints, quality_attributes];
}

impl Parts for Requirement {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    /// A statement enters as its rendered sentence (AC-598).
    fn fields(&self) -> Fields<'_> {
        let details = &self.details;
        plain(
            NodeKind::Requirement,
            self.statement.render(),
            details.body.as_deref(),
            &details.tags,
        )
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        grouped!(visit, &self.details.sections, &self.criteria)
    }
}

impl Parts for Constraint {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        Fields {
            binding: self.binding.as_ref(),
            conservation_kind: self.conservation_kind,
            verification_fate: self.verification_fate,
            ..plain(
                NodeKind::Constraint,
                &self.name,
                self.details.body.as_deref(),
                &self.details.tags,
            )
        }
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        grouped!(visit, &self.details.sections)
    }
}

impl Parts for QualityAttribute {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        Fields {
            qa_kind: Some(self.kind),
            quality_target: Some(&self.target),
            verification_fate: self.verification_fate,
            ..plain(
                NodeKind::QualityAttribute,
                &self.name,
                self.details.body.as_deref(),
                &self.details.tags,
            )
        }
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        grouped!(visit, &self.details.sections)
    }
}

impl Parts for Section {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        plain(NodeKind::Section, &self.name, Some(&self.body), &[])
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        grouped!(visit, &self.sections)
    }
}

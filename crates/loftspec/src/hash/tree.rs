//! Node content views and content hashes (RQ-11) over the typed tree.

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
    Component, ComponentChild, Constraint, Criterion, Details, Feature, FeatureGroup, Product,
    QualityAttribute, Requirement, Section,
};

/// Called once per node, children first, with the node's ID, content view and content hash.
pub(crate) type Visit<'a> = dyn FnMut(Option<NodeId>, Value, Digest) -> Result<(), HashError> + 'a;

mod sealed {
    use super::{
        ConservationKind, Digest, HashError, NodeId, NodeKind, QaKind, QualityTarget, TagName,
        TechnicalBinding, VerificationFate, Visit,
    };

    /// The fields of a node's content view other than its children.
    pub struct Fields<'a> {
        pub kind: NodeKind,
        pub text: String,
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

fn plain<'a>(kind: NodeKind, text: &str, details: &'a Details) -> Fields<'a> {
    Fields {
        kind,
        text: text.to_owned(),
        body: details.body.as_deref(),
        tags: &details.tags,
        qa_kind: None,
        quality_target: None,
        binding: None,
        conservation_kind: None,
        verification_fate: None,
    }
}

/// The structural children's hashes in order, then the Sections' (structural children first).
fn hashes<'a, C: Parts + 'a>(
    children: impl IntoIterator<Item = &'a C>,
    sections: &[Section],
    visit: &mut Visit<'_>,
) -> Result<Vec<Digest>, HashError> {
    let mut hashes = children
        .into_iter()
        .map(|child| hash_with(child, visit))
        .collect::<Result<Vec<_>, _>>()?;
    for section in sections {
        hashes.push(hash_with(section, visit)?);
    }
    Ok(hashes)
}

impl Parts for Product {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        plain(NodeKind::Product, &self.name, &self.details)
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes(&self.feature_groups, &self.details.sections, visit)
    }
}

impl Parts for FeatureGroup {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        plain(NodeKind::FeatureGroup, &self.name, &self.details)
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes(&self.features, &self.details.sections, visit)
    }
}

impl Parts for Feature {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        plain(NodeKind::Feature, &self.name, &self.details)
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes(&self.requirements, &self.details.sections, visit)
    }
}

impl Parts for Requirement {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    /// A statement enters as its rendered sentence (AC-242).
    fn fields(&self) -> Fields<'_> {
        plain(
            NodeKind::Requirement,
            &self.statement.render(),
            &self.details,
        )
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes(&self.criteria, &self.details.sections, visit)
    }
}

impl Parts for Criterion {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        plain(NodeKind::Criterion, &self.name, &self.details)
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes::<Section>([], &self.details.sections, visit)
    }
}

impl Parts for Component {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        plain(NodeKind::Component, &self.name, &self.details)
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes(&self.children, &self.details.sections, visit)
    }
}

impl Parts for ComponentChild {
    fn id(&self) -> Option<NodeId> {
        match self {
            Self::Component(node) => node.id(),
            Self::Constraint(node) => node.id(),
            Self::QualityAttribute(node) => node.id(),
        }
    }
    fn fields(&self) -> Fields<'_> {
        match self {
            Self::Component(node) => node.fields(),
            Self::Constraint(node) => node.fields(),
            Self::QualityAttribute(node) => node.fields(),
        }
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        match self {
            Self::Component(node) => node.child_hashes(visit),
            Self::Constraint(node) => node.child_hashes(visit),
            Self::QualityAttribute(node) => node.child_hashes(visit),
        }
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
            ..plain(NodeKind::Constraint, &self.name, &self.details)
        }
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes::<Section>([], &self.details.sections, visit)
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
            ..plain(NodeKind::QualityAttribute, &self.name, &self.details)
        }
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes::<Section>([], &self.details.sections, visit)
    }
}

impl Parts for Section {
    fn id(&self) -> Option<NodeId> {
        self.id.map(NodeId::from)
    }
    fn fields(&self) -> Fields<'_> {
        Fields {
            kind: NodeKind::Section,
            text: self.name.clone(),
            body: Some(&self.body),
            tags: &[],
            qa_kind: None,
            quality_target: None,
            binding: None,
            conservation_kind: None,
            verification_fate: None,
        }
    }
    fn child_hashes(&self, visit: &mut Visit<'_>) -> Result<Vec<Digest>, HashError> {
        hashes::<Section>([], &self.sections, visit)
    }
}

//! Conversion from a Loft v1 Authored State export (D-11 F-13..F-16).

pub mod export;
mod links;
mod prose;
pub mod sidecar;
mod tree;

use uuid::Uuid;

use self::export::{Export, ExternalId};
use self::sidecar::V1Sidecar;
use crate::bindings::{BindingError, Counters};
use crate::document::{CategoryEntry, DerivedFrom, Document, TagEntry};
use crate::identity::{Label, LabelError};
use crate::ids::{
    AuthoredStateId, CategoryId, DecisionId, DocumentId, NodeId, RequirementId, TagId,
};
use crate::names::NameError;

/// A converted document and what the converter reported about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    pub document: Document,
    pub sidecar: V1Sidecar,
    pub findings: Vec<Finding>,
}

/// Something the converter kept as written but reports, so an author can improve it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A Requirement's name is not one of the five forms as its template writes it, so it is
    /// stored as text (AC-187).
    StatementAsText { requirement: RequirementId },
    /// A decision names a node the export does not hold, as a v1 decision may after the node
    /// was deleted. The reference is kept; how the Format holds it is open (D-11 gap G19).
    AbsentSubject {
        decision: DecisionId,
        subject: NodeId,
    },
    /// A label reference in a node's name or body resolves to no element (AC-191).
    UnresolvedReference { node: NodeId, reference: Label },
    /// A body holds a Markdown heading, at this 1-based line (AC-193).
    HeadingInBody { node: NodeId, line: usize },
}

/// An export the converter refuses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    #[error("the export names no document: it holds no node, registry entry or decision")]
    NoDocument,
    #[error("the export names two documents, {first} and {second}")]
    TwoDocuments { first: Uuid, second: Uuid },
    #[error("the export carries annotations, which the Format does not hold")]
    Annotations,
    #[error("{parent} lists child {child}, which the export does not hold")]
    UnknownChild { parent: ExternalId, child: Uuid },
    #[error("{child} is listed by both {first} and {second}")]
    TwoParents {
        child: ExternalId,
        first: ExternalId,
        second: ExternalId,
    },
    #[error("{node} is listed by no parent, and only a Product or a Component is a root")]
    Orphan { node: ExternalId },
    #[error("{child} cannot stand under {parent}")]
    Containment {
        parent: ExternalId,
        child: ExternalId,
    },
    #[error("{node} has no {field}")]
    Missing {
        node: ExternalId,
        field: &'static str,
    },
    #[error("{node}: {source}")]
    Label {
        node: ExternalId,
        source: LabelError,
    },
    #[error("Facet Tag {node} has a body, which a tag does not hold")]
    FacetTagBody { node: ExternalId },
    #[error("Facet Tag {node}: {source}")]
    Tag { node: ExternalId, source: NameError },
    #[error(
        "a Realization Link from {feature} to {component} does not join a Feature to a Component"
    )]
    Link { feature: Uuid, component: Uuid },
    #[error("{node} is scoped to {product}, which is not a Product of the export")]
    Scope { node: ExternalId, product: Uuid },
    #[error("{decision} names {subject}, which is not a node of any kind the Format holds")]
    DecisionSubject { decision: ExternalId, subject: Uuid },
    #[error("element {element} of {decision} {reason}")]
    DecisionElement {
        decision: ExternalId,
        element: Uuid,
        reason: &'static str,
    },
    #[error(transparent)]
    Binding(#[from] BindingError),
}

/// Converts a v1 export into a document. The document key is the v1 document UUID that every
/// element of the export carries.
pub fn convert(export: &Export) -> Result<Conversion, ConvertError> {
    if !export.annotations.is_empty() {
        return Err(ConvertError::Annotations);
    }
    let mut document = Document::empty(DocumentId::new(document_uuid(export)?.into()));
    let mut findings = Vec::new();
    tree::Tree::new(export)?.convert(&mut document, &mut findings)?;
    let nodes = tree::node_ids(export)?;
    document.edges = links::realizations(export, &nodes)?;
    document.edges.extend(links::scopes(export, &nodes)?);
    links::decisions(export, &nodes, &mut document, &mut findings)?;
    let (mentions, prose_findings) = prose::mentions(export, &nodes, &document.bindings);
    document.edges.extend(mentions);
    findings.extend(prose_findings);
    document.derived_from = Some(DerivedFrom {
        authored_state_id: AuthoredStateId::new(export.authored_state_id.into()),
        content_address: export.content_address,
        publish_sequence: export.publish_sequence,
    });
    document.tags = export
        .vocabulary_entries
        .iter()
        .map(|entry| TagEntry {
            id: TagId::new(entry.entry_id.into()),
            name: entry.name.clone(),
            definition: entry.definition.clone(),
        })
        .collect();
    document.categories = export
        .technical_constraint_categories
        .iter()
        .map(|category| CategoryEntry {
            id: CategoryId::new(category.id.into()),
            name: category.name.clone(),
            definition: category.definition.clone(),
        })
        .collect();
    let sidecar = V1Sidecar::collect(export, document.id, &nodes);
    document.counters = Counters::above(&document.bindings);
    // A label a decision still names is seen even when its node is gone, so it is not reused.
    for external_id in sidecar.absent_subjects.values() {
        if let Ok(label) = Label::parse(&external_id.to_string()) {
            document.counters.raise(label);
        }
    }
    Ok(Conversion {
        document,
        sidecar,
        findings,
    })
}

/// The one document UUID the export's elements carry (D-11 gap G18).
fn document_uuid(export: &Export) -> Result<Uuid, ConvertError> {
    let nodes = export.nodes.iter().map(|node| node.document_id);
    let entries = export
        .vocabulary_entries
        .iter()
        .map(|entry| entry.document_id);
    let categories = export
        .technical_constraint_categories
        .iter()
        .map(|category| category.document_id);
    let decisions = export.architecture_decisions.iter().flat_map(|decision| {
        std::iter::once(decision.identity.document_id)
            .chain(decision.elements.iter().map(|element| element.document_id))
    });
    let mut uuids = nodes.chain(entries).chain(categories).chain(decisions);
    let first = uuids.next().ok_or(ConvertError::NoDocument)?;
    match uuids.find(|uuid| *uuid != first) {
        Some(second) => Err(ConvertError::TwoDocuments { first, second }),
        None => Ok(first),
    }
}

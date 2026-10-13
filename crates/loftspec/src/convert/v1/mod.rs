//! Conversion from a Loft v1 Authored State export (F-13..F-16).

pub mod export;
mod links;
mod prose;
pub mod sidecar;
mod tree;

use uuid::Uuid;

use self::export::{Export, ExternalId};
use self::sidecar::V1Sidecar;
use self::tree::Tree;
use crate::bindings::{BindingError, Counters};
use crate::document::{CategoryEntry, DerivedFrom, Document, TagEntry};
use crate::identity::{Label, LabelError};
use crate::ids::{
    AuthoredStateId, CategoryId, ComponentId, DocumentId, NodeId, RequirementId, TagId,
};
use crate::names::NameError;
use crate::slug::Slug;

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
    /// A label reference in a node's name or body resolves to no element (AC-191).
    UnresolvedReference { node: NodeId, reference: Label },
    /// A body holds a Markdown heading, at this 1-based line (AC-193).
    HeadingInBody { node: NodeId, line: usize },
    /// A Component scoped to a Product or the document beneath a Component scoped as external:
    /// v1 treats it as internal, and v2 gives it its ancestor's boundary (ADR-15).
    InternalUnderExternal {
        component: ComponentId,
        ancestor: ComponentId,
    },
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
    let document_id = DocumentId::new(document_uuid(export)?.into());
    let tree = Tree::new(export)?;
    let mut conversion = Conversion {
        document: Document::empty(document_id),
        sidecar: V1Sidecar::new(document_id),
        findings: Vec::new(),
    };
    tree.convert(&mut conversion)?;
    links::convert(export, &tree.ids, &mut conversion)?;
    let (mentions, findings) = prose::mentions(export, &tree.ids, &conversion.document.bindings);
    conversion.document.edges.extend(mentions);
    conversion.findings.extend(findings);
    conversion.sidecar.record_highest(export);
    registries(export, &mut conversion.document);
    let document = &mut conversion.document;
    unique_slugs(document);
    document.counters = Counters::above(&document.bindings);
    // A former label a decision names is seen, so it is never allocated again (ADR-13).
    for former in conversion.sidecar.former_labels.keys() {
        document.counters.raise(*former);
    }
    Ok(conversion)
}

/// Makes the slugs of each directory unique, in document order (AC-246): the Products, each
/// Product's Feature Groups, each Feature Group's Features, the root Components and the
/// decisions.
fn unique_slugs(document: &mut Document) {
    Slug::make_unique(
        document
            .products
            .iter_mut()
            .map(|product| &mut product.slug),
    );
    for product in &mut document.products {
        Slug::make_unique(
            product
                .feature_groups
                .iter_mut()
                .map(|group| &mut group.slug),
        );
        for group in &mut product.feature_groups {
            Slug::make_unique(group.features.iter_mut().map(|feature| &mut feature.slug));
        }
    }
    Slug::make_unique(document.components.iter_mut().map(|root| &mut root.slug));
    Slug::make_unique(
        document
            .decisions
            .iter_mut()
            .map(|decision| &mut decision.slug),
    );
}

/// The label an external ID is, or an error naming the node that carries it.
fn label(external_id: &ExternalId) -> Result<Label, ConvertError> {
    external_id.label().map_err(|source| ConvertError::Label {
        node: external_id.clone(),
        source,
    })
}

/// The source record, the tag registry and the category registry.
fn registries(export: &Export, document: &mut Document) {
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
}

/// The one document UUID the export's elements carry.
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

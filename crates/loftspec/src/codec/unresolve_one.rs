//! One record of a document as the writer writes it, built alone: a node with the edges it
//! writes, a decision, a registry entry or a binding, each found by its kind and key, in
//! canonical layout. Only the record's own edges are gathered, and the nodes it holds are left
//! out of it, as its own record view leaves them out.

use super::canon::sort;
use super::own_record::RecordKind;
use super::record::Rec;
use super::schema::{Kind, field};
use super::unresolve::Edges;
use super::unresolve_singles::{binding_record, category_record, tag_record, term_record};
use crate::document::Document;
use crate::identity::Key;
use crate::kind::NodeKind;
use crate::node::Node;

/// The record of kind `kind` keyed `key`, as the writer writes it; none where the document holds
/// no such record. A binding is found by the key of the element it binds.
pub(super) fn written_record(document: &Document, kind: RecordKind, key: Key) -> Option<Rec> {
    let mut record = match kind {
        RecordKind::Node(node) => node_record(document, node, key)?,
        RecordKind::Decision => {
            let decision = document
                .decisions
                .iter()
                .find(|each| each.id.key() == key)?;
            edges_of(document, key).decision(decision)
        }
        RecordKind::Tag => tag_record(document.tags.iter().find(|tag| tag.id.key() == key)?),
        RecordKind::Category => {
            let found = document.categories.iter().find(|each| each.id.key() == key);
            category_record(found?)
        }
        RecordKind::Term => term_record(document.terms.iter().find(|term| term.id.key() == key)?),
        RecordKind::Binding => binding_record(document.bindings.get(key)?),
    };
    sort(&mut record);
    Some(record)
}

/// The edges the element keyed `key` writes, building its record without the nodes it holds.
fn edges_of(document: &Document, key: Key) -> Edges {
    let written = document.edges.iter();
    Edges::of(written.filter(|edge| edge.source().key() == key)).own()
}

fn node_record(document: &Document, kind: NodeKind, key: Key) -> Option<Rec> {
    let root = document.components.iter().find(|root| {
        kind == NodeKind::Component && root.component.id.is_some_and(|id| id.key() == key)
    });
    if let Some(root) = root {
        let mut record = edges_of(document, key).component(&root.component);
        record.kind = Kind::RootComponent;
        record.set_text(field::SLUG, root.slug.as_str());
        return Some(record);
    }
    let node = find(document, kind, key)?;
    let edges = edges_of(document, key);
    Some(match node {
        Node::Product(product) => edges.product(product),
        Node::FeatureGroup(group) => edges.group(group),
        Node::Feature(feature) => edges.feature(feature),
        Node::Requirement(requirement) => edges.requirement(requirement),
        Node::Criterion(criterion) => edges.criterion(criterion),
        Node::Component(component) => edges.component(component),
        Node::Constraint(constraint) => edges.constraint(constraint),
        Node::QualityAttribute(quality) => edges.quality(quality),
        Node::Section(section) => edges.section(section),
    })
}

/// The first node of kind `kind` keyed `key`, parents before children, by a walk that stops
/// there.
fn find(document: &Document, kind: NodeKind, key: Key) -> Option<Node<'_>> {
    let mut stack: Vec<Node<'_>> = document.root_nodes().collect();
    stack.reverse();
    while let Some(node) = stack.pop() {
        if node
            .id()
            .is_some_and(|id| id.kind() == kind && id.key() == key)
        {
            return Some(node);
        }
        stack.extend(node.children().into_iter().rev());
    }
    None
}

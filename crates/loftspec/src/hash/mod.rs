//! The hasher (CMP-3): content hashes, decision view hashes and the roots of a document (D-11
//! F-3, F-4, F-5). Each view is RFC 8785 canonical JSON; each digest is domain-separated.

mod decisions;
mod tree;

use std::collections::BTreeMap;

use serde_json::{Value, json};

pub use tree::ContentHash;

use crate::FORMAT;
use crate::canonical::{Digest, Domain, canonical_bytes, digest};
use crate::document::{DerivedFrom, Document, Edge, SubjectRole};
use crate::identity::Key;
use crate::ids::{DecisionId, ElementId, NodeId};
use crate::manifest::ALGORITHM;

/// A document the hasher cannot compute roots for.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HashError {
    #[error("key {0} is carried by two elements of the document")]
    DuplicateKey(Key),
    #[error("an edge names {0}, which the document does not hold")]
    UnknownEdgeEndpoint(ElementId),
    #[error("the bindings name {0}, which the document does not hold")]
    UnknownBindingElement(ElementId),
    #[error("decision {decision} refers to key {reference}, which it cannot resolve")]
    UnknownDecisionReference {
        decision: DecisionId,
        reference: Key,
    },
    #[error("decision {0} is not in the document")]
    UnknownDecision(DecisionId),
    #[error("decision {0} supersedes itself through its successors")]
    SupersessionCycle(DecisionId),
}

/// The content hash of every keyed node, by key, with the node's typed ID.
pub(crate) type NodeIndex = BTreeMap<Key, (NodeId, Digest)>;

/// The content hash of a node, if the index holds a node of that kind under that key.
pub(crate) fn node_hash(index: &NodeIndex, id: NodeId) -> Option<Digest> {
    index
        .get(&id.key())
        .filter(|(held, _)| *held == id)
        .map(|(_, hash)| *hash)
}

/// The roots of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roots {
    pub edges: Digest,
    pub tags: Digest,
    pub categories: Digest,
    pub decisions: Digest,
    pub content: Digest,
    pub bindings: Digest,
    pub state: Digest,
}

/// Every hash the hasher computes for one document, with the views it digested.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentHashes {
    /// The content hash of every keyed node.
    pub nodes: BTreeMap<NodeId, Digest>,
    /// The content view of every keyed node.
    pub node_views: BTreeMap<NodeId, Value>,
    /// The view hash of every decision.
    pub decisions: BTreeMap<DecisionId, Digest>,
    /// The view of every decision.
    pub decision_views: BTreeMap<DecisionId, Value>,
    /// The Product roots in the manifest's order.
    pub products: Vec<Digest>,
    /// The root Component roots, sorted.
    pub components: Vec<Digest>,
    pub roots: Roots,
    /// Each root's name, domain and view, in the order the roots are computed.
    pub root_views: Vec<(&'static str, Domain, Value)>,
}

struct Trees {
    index: NodeIndex,
    views: BTreeMap<NodeId, Value>,
    products: Vec<Digest>,
    components: Vec<Digest>,
}

fn hash_trees(document: &Document) -> Result<Trees, HashError> {
    let mut index = NodeIndex::new();
    let mut views = BTreeMap::new();
    let mut visit = |id: Option<NodeId>, view: Value, hash: Digest| {
        if let Some(id) = id {
            if index.insert(id.key(), (id, hash)).is_some() {
                return Err(HashError::DuplicateKey(id.key()));
            }
            views.insert(id, view);
        }
        Ok(())
    };
    let products = document
        .products
        .iter()
        .map(|product| tree::hash_with(product, &mut visit))
        .collect::<Result<Vec<_>, _>>()?;
    let mut components = document
        .components
        .iter()
        .map(|component| tree::hash_with(component, &mut visit))
        .collect::<Result<Vec<_>, _>>()?;
    components.sort_unstable();
    Ok(Trees {
        index,
        views,
        products,
        components,
    })
}

/// Computes every content hash, decision view hash and root of a document.
pub fn hash_document(document: &Document) -> Result<DocumentHashes, HashError> {
    let trees = hash_trees(document)?;
    let (decisions, decision_views) =
        decisions::DecisionHasher::new(&document.decisions, &trees.index)?.hash_all()?;
    let resolve = |element: ElementId| match element {
        ElementId::Node(id) => node_hash(&trees.index, id),
        ElementId::Decision(id) => decisions.get(&id).copied(),
    };
    let mut views = Vec::new();
    let mut root = |name: &'static str, domain: Domain, view: Value| {
        let hash = digest(domain, &view);
        views.push((name, domain, view));
        hash
    };
    let edges = root(
        "edges",
        Domain::Edges,
        edges_view(&document.edges, &resolve)?,
    );
    let tag_rows = document
        .tags
        .iter()
        .map(|tag| [tag.name.as_str(), tag.definition.as_str()]);
    let tags = root("tags", Domain::Registry, registry_view("tags", tag_rows));
    let category_rows = document
        .categories
        .iter()
        .map(|category| [category.name.as_str(), category.definition.as_str()]);
    let categories = root(
        "categories",
        Domain::Registry,
        registry_view("categories", category_rows),
    );
    let mut decision_hashes: Vec<Digest> = decisions.values().copied().collect();
    decision_hashes.sort_unstable();
    let decisions_root = root("decisions", Domain::Decisions, json!(decision_hashes));
    let content = root(
        "content",
        Domain::Content,
        json!([
            trees.products,
            trees.components,
            edges,
            tags,
            categories,
            decisions_root
        ]),
    );
    let bindings = root(
        "bindings",
        Domain::Bindings,
        bindings_view(document, &resolve)?,
    );
    let state = root(
        "state",
        Domain::State,
        state_view(document, content, bindings),
    );
    let roots = Roots {
        edges,
        tags,
        categories,
        decisions: decisions_root,
        content,
        bindings,
        state,
    };
    Ok(DocumentHashes {
        nodes: trees.index.values().copied().collect(),
        node_views: trees.views,
        decisions,
        decision_views,
        products: trees.products,
        components: trees.components,
        roots,
        root_views: views,
    })
}

/// The edges root view: each edge as its source's hash, its type, its target's hash and its
/// role, the tuples sorted by their canonical bytes, duplicates kept (RQ-13). An endpoint's hash
/// is a node's content hash or a decision's view hash.
fn edges_view(
    edges: &[Edge],
    resolve: &impl Fn(ElementId) -> Option<Digest>,
) -> Result<Value, HashError> {
    let endpoint =
        |element: ElementId| resolve(element).ok_or(HashError::UnknownEdgeEndpoint(element));
    let mut tuples = edges
        .iter()
        .map(|edge| {
            let tuple = json!([
                endpoint(edge.source())?,
                edge.kind().as_str(),
                endpoint(edge.target())?,
                edge.role().map(SubjectRole::as_str),
            ]);
            Ok((canonical_bytes(&tuple), tuple))
        })
        .collect::<Result<Vec<_>, HashError>>()?;
    tuples.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    Ok(Value::Array(
        tuples.into_iter().map(|(_, tuple)| tuple).collect(),
    ))
}

/// A registry root view: the registry's name and its entries as name and definition, sorted by
/// name; entry keys are not content (RQ-61).
fn registry_view<'a>(name: &str, entries: impl Iterator<Item = [&'a str; 2]>) -> Value {
    let mut rows: Vec<[&str; 2]> = entries.collect();
    rows.sort_unstable();
    json!([name, rows])
}

/// The binding root view: key, live label, aliases and computed hash, sorted by key (RQ-15,
/// AC-240).
fn bindings_view(
    document: &Document,
    resolve: &impl Fn(ElementId) -> Option<Digest>,
) -> Result<Value, HashError> {
    let rows = document
        .bindings
        .iter()
        .map(|binding| {
            let hash = resolve(binding.element)
                .ok_or(HashError::UnknownBindingElement(binding.element))?;
            Ok(json!([
                binding.element.key(),
                binding.label,
                binding.aliases,
                hash
            ]))
        })
        .collect::<Result<Vec<_>, HashError>>()?;
    Ok(Value::Array(rows))
}

/// The state view: format, algorithm, document key, content root, binding root and the
/// derived-from record (AC-62).
fn state_view(document: &Document, content: Digest, bindings: Digest) -> Value {
    let derived_from = document.derived_from.as_ref().map(
        |DerivedFrom {
             authored_state_id,
             content_address,
             publish_sequence,
         }| {
            json!({
                "authoredStateId": authored_state_id,
                "contentAddress": content_address,
                "publishSequence": publish_sequence,
            })
        },
    );
    json!([
        FORMAT,
        ALGORITHM,
        document.id,
        content,
        bindings,
        derived_from
    ])
}

//! The hasher (CMP-3): content hashes, decision view hashes and the roots of a document (D-11
//! F-3, F-4, F-5). Each view is RFC 8785 canonical JSON; each digest is domain-separated.

mod decisions;
mod tree;

use std::collections::BTreeMap;

use serde_json::{Value, json};

pub use tree::{content_hash, node_view};

use crate::FORMAT;
use crate::canonical::{Digest, Domain, canonical_bytes, digest};
use crate::identity::Key;
use crate::manifest::ALGORITHM;
use crate::model::{DerivedFrom, Document, Edge, RegistryEntry};

/// A document the hasher cannot compute roots for.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HashError {
    #[error("key {0} is carried by two elements of the document")]
    DuplicateKey(Key),
    #[error("an edge names key {0}, which is neither a node nor a decision of the document")]
    UnknownEdgeEndpoint(Key),
    #[error("the bindings name key {0}, which is neither a node nor a decision of the document")]
    UnknownBindingKey(Key),
    #[error("decision {decision} refers to key {reference}, which it cannot resolve")]
    UnknownDecisionReference { decision: Key, reference: Key },
    #[error("decision {0} is not in the document")]
    UnknownDecision(Key),
    #[error("decision {0} supersedes itself through its successors")]
    SupersessionCycle(Key),
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

/// Every hash the hasher computes for one document, with the root views it digested.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentHashes {
    /// The content hash of every keyed node.
    pub nodes: BTreeMap<Key, Digest>,
    /// The view hash of every decision.
    pub decisions: BTreeMap<Key, Digest>,
    /// The view of every decision.
    pub decision_views: BTreeMap<Key, Value>,
    /// The Product roots in the manifest's order.
    pub products: Vec<Digest>,
    /// The root Component roots, sorted.
    pub components: Vec<Digest>,
    pub roots: Roots,
    /// Each root's view and domain, in the order the roots are computed.
    pub root_views: Vec<(&'static str, Domain, Value)>,
}

/// Computes every content hash, decision view hash and root of a document.
pub fn hash_document(document: &Document) -> Result<DocumentHashes, HashError> {
    let mut nodes = BTreeMap::new();
    let products = document
        .products
        .iter()
        .map(|product| tree::index_tree(product, &mut nodes))
        .collect::<Result<Vec<_>, _>>()?;
    let mut components = document
        .components
        .iter()
        .map(|component| tree::index_tree(component, &mut nodes))
        .collect::<Result<Vec<_>, _>>()?;
    components.sort_unstable();
    let (decisions, decision_views) =
        decisions::DecisionHasher::new(&document.decisions, &nodes)?.hash_all()?;
    let resolve = |key: Key| nodes.get(&key).or_else(|| decisions.get(&key)).copied();
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
    let tags = root(
        "tags",
        Domain::Registry,
        registry_view("tags", &document.tags),
    );
    let categories = root(
        "categories",
        Domain::Registry,
        registry_view("categories", &document.categories),
    );
    let mut decision_hashes: Vec<Digest> = decisions.values().copied().collect();
    decision_hashes.sort_unstable();
    let decisions_root = root("decisions", Domain::Decisions, json!(decision_hashes));
    let content = root(
        "content",
        Domain::Content,
        json!([
            products,
            components,
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
        nodes,
        decisions,
        decision_views,
        products,
        components,
        roots,
        root_views: views,
    })
}

/// The edges root view: each edge as its source's content hash, its type, its target's content
/// hash and its role, the tuples sorted by their canonical bytes, duplicates kept (RQ-13).
fn edges_view(
    edges: &[Edge],
    resolve: &impl Fn(Key) -> Option<Digest>,
) -> Result<Value, HashError> {
    let endpoint = |key: Key| resolve(key).ok_or(HashError::UnknownEdgeEndpoint(key));
    let mut tuples = edges
        .iter()
        .map(|edge| {
            let tuple = json!([
                endpoint(edge.source)?,
                edge.kind.as_str(),
                endpoint(edge.target)?,
                edge.role.map(crate::model::SubjectRole::as_str),
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
fn registry_view(name: &str, entries: &[RegistryEntry]) -> Value {
    let mut rows: Vec<[&str; 2]> = entries
        .iter()
        .map(|entry| [entry.name.as_str(), entry.definition.as_str()])
        .collect();
    rows.sort_unstable();
    json!([name, rows])
}

/// The binding root view: key, live label, aliases and computed content hash, sorted by key
/// (RQ-15, AC-240).
fn bindings_view(
    document: &Document,
    resolve: &impl Fn(Key) -> Option<Digest>,
) -> Result<Value, HashError> {
    let rows = document
        .bindings
        .iter()
        .map(|(key, binding)| {
            let hash = resolve(key).ok_or(HashError::UnknownBindingKey(key))?;
            Ok(json!([key, binding.label, binding.aliases, hash]))
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
        document.key,
        content,
        bindings,
        derived_from
    ])
}

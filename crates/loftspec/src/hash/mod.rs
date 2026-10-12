//! The hasher (CMP-3): content hashes, decision view hashes and the roots of a document (D-11
//! F-3, F-4, F-5). Each view is RFC 8785 canonical JSON; each digest is domain-separated. The
//! hasher takes an indexed document, so every reference it follows resolves.

mod decisions;
mod tree;

use std::collections::BTreeMap;

use serde_json::{Value, json};

pub use tree::ContentHash;

use crate::FORMAT;
use crate::canonical::{Digest, Domain, canonical_bytes, digest_array};
use crate::document::{DerivedFrom, Document, Edge, EdgeTarget, SubjectRole};
use crate::identity::{Key, Label};
use crate::ids::{DecisionId, ElementId, NodeId};
use crate::index::{IndexErrors, Indexed};
use crate::manifest::ALGORITHM;

/// The content hash of every keyed node, by key.
pub(crate) type NodeIndex = BTreeMap<Key, Digest>;

/// The content hash of a node of an indexed document.
pub(crate) fn node_hash(index: &NodeIndex, id: NodeId) -> Digest {
    *index
        .get(&id.key())
        .expect("an indexed document holds every node it names")
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

/// Every hash the hasher computes for one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHashes {
    /// The content hash of every keyed node.
    pub nodes: BTreeMap<NodeId, Digest>,
    /// The view hash of every decision.
    pub decisions: BTreeMap<DecisionId, Digest>,
    /// The Product roots in the manifest's order.
    pub products: Vec<Digest>,
    /// The root Component roots, sorted.
    pub components: Vec<Digest>,
    pub roots: Roots,
}

/// The views the hasher digested, kept only when a caller asks for them: the projections and
/// the golden vectors read them, a publish or a verification does not.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Views {
    /// The content view of every keyed node.
    pub nodes: BTreeMap<NodeId, Value>,
    /// The view of every decision.
    pub decisions: BTreeMap<DecisionId, Value>,
    /// Each root's name, domain and view, in the order the roots are computed.
    pub roots: Vec<(&'static str, Domain, Value)>,
}

struct Trees {
    nodes: BTreeMap<NodeId, Digest>,
    products: Vec<Digest>,
    components: Vec<Digest>,
}

impl Trees {
    fn index(&self) -> NodeIndex {
        self.nodes
            .iter()
            .map(|(id, hash)| (id.key(), *hash))
            .collect()
    }
}

fn hash_trees(document: &Document, mut views: Option<&mut BTreeMap<NodeId, Value>>) -> Trees {
    let mut nodes = BTreeMap::new();
    let mut visit = |id: Option<NodeId>, view: &tree::NodeView<'_>, hash: Digest| {
        if let Some(id) = id {
            nodes.insert(id, hash);
            if let Some(views) = views.as_deref_mut() {
                views.insert(id, view.to_value());
            }
        }
    };
    let products = document
        .products
        .iter()
        .map(|product| tree::hash_with(product.into(), &mut visit))
        .collect();
    let mut components: Vec<Digest> = document
        .components
        .iter()
        .map(|root| tree::hash_with((&root.component).into(), &mut visit))
        .collect();
    components.sort_unstable();
    Trees {
        nodes,
        products,
        components,
    }
}

/// Indexes a document, then computes every content hash, decision view hash and root.
///
/// # Errors
///
/// Every integrity error the document's index finds.
pub fn hash_document(document: &Document) -> Result<DocumentHashes, IndexErrors> {
    Ok(hash_indexed(&document.index()?))
}

/// [`hash_document`], keeping every view it digested.
///
/// # Errors
///
/// Every integrity error the document's index finds.
pub fn hash_document_with_views(
    document: &Document,
) -> Result<(DocumentHashes, Views), IndexErrors> {
    Ok(hash_indexed_with_views(&document.index()?))
}

/// Computes every content hash, decision view hash and root of an indexed document.
#[must_use]
pub fn hash_indexed(indexed: &Indexed<'_>) -> DocumentHashes {
    hash(indexed, None)
}

/// [`hash_indexed`], keeping every view it digested.
#[must_use]
pub fn hash_indexed_with_views(indexed: &Indexed<'_>) -> (DocumentHashes, Views) {
    let mut views = Views::default();
    let hashes = hash(indexed, Some(&mut views));
    (hashes, views)
}

fn hash(indexed: &Indexed<'_>, mut views: Option<&mut Views>) -> DocumentHashes {
    let document = indexed.document();
    let trees = hash_trees(document, views.as_deref_mut().map(|views| &mut views.nodes));
    let index = trees.index();
    let (decisions, decision_views) =
        decisions::DecisionHasher::new(&document.decisions, &index, views.is_some()).hash_all();
    let resolve = |element: ElementId| match element {
        ElementId::Node(id) => node_hash(&index, id),
        ElementId::Decision(id) => decisions[&id],
    };
    let mut kept = Vec::new();
    let mut root = |name: &'static str, domain: Domain, view: &dyn erased::View| {
        let hash = view.digest(domain);
        if views.is_some() {
            kept.push((name, domain, view.value()));
        }
        hash
    };
    let edges = root(
        "edges",
        Domain::Edges,
        &EdgesView::of(&document.edges, &resolve),
    );
    let tag_rows = document
        .tags
        .iter()
        .map(|tag| [tag.name.as_str(), tag.definition.as_str()]);
    let tags = root("tags", Domain::Registry, &registry_view("tags", tag_rows));
    let category_rows = document
        .categories
        .iter()
        .map(|category| [category.name.as_str(), category.definition.as_str()]);
    let categories = root(
        "categories",
        Domain::Registry,
        &registry_view("categories", category_rows),
    );
    let mut decision_hashes: Vec<Digest> = decisions.values().copied().collect();
    decision_hashes.sort_unstable();
    let decisions_root = root("decisions", Domain::Decisions, &decision_hashes);
    let content_view = (
        &trees.products,
        &trees.components,
        edges,
        tags,
        categories,
        decisions_root,
    );
    let content = root("content", Domain::Content, &content_view);
    let bindings = root(
        "bindings",
        Domain::Bindings,
        &bindings_view(document, &resolve),
    );
    let state = root(
        "state",
        Domain::State,
        &state_view(document, content, bindings),
    );
    if let Some(views) = views {
        views.decisions = decision_views;
        views.roots = kept;
    }
    DocumentHashes {
        nodes: trees.nodes,
        decisions,
        products: trees.products,
        components: trees.components,
        roots: Roots {
            edges,
            tags,
            categories,
            decisions: decisions_root,
            content,
            bindings,
            state,
        },
    }
}

/// A root view, digested by streaming it, or by its own means, and turned into a value only when
/// a caller keeps it.
mod erased {
    use serde::Serialize;
    use serde_json::Value;

    use crate::canonical::{Digest, Domain, digest};

    pub(super) trait View {
        fn digest(&self, domain: Domain) -> Digest;
        fn value(&self) -> Value;
    }

    impl<T: Serialize> View for T {
        fn digest(&self, domain: Domain) -> Digest {
            digest(domain, self)
        }

        fn value(&self) -> Value {
            serde_json::to_value(self).expect("a root view serialises")
        }
    }
}

/// How a view names a node the document no longer holds: `{"formerLabel": <label>}` (ADR-13).
pub(crate) fn former(label: Label) -> Value {
    json!({ "formerLabel": label })
}

/// The edges root view: each edge as its source's hash, its type, its target's hash and its
/// role, the tuples sorted by their canonical bytes, duplicates kept (RQ-13). An endpoint's hash
/// is a node's content hash or a decision's view hash; a former label enters as itself. Each
/// tuple is canonicalised once, and the sorted bytes are digested as the array they make.
struct EdgesView(Vec<Vec<u8>>);

impl EdgesView {
    fn of(edges: &[Edge], resolve: &impl Fn(ElementId) -> Digest) -> Self {
        let mut tuples: Vec<Vec<u8>> = edges
            .iter()
            .map(|edge| {
                let target = match edge.target() {
                    EdgeTarget::Element(element) => json!(resolve(element)),
                    EdgeTarget::Former(label) => former(label),
                };
                canonical_bytes(&(
                    resolve(edge.source()),
                    edge.kind().as_str(),
                    target,
                    edge.role().map(SubjectRole::as_str),
                ))
            })
            .collect();
        tuples.sort_unstable();
        Self(tuples)
    }
}

impl erased::View for EdgesView {
    fn digest(&self, domain: Domain) -> Digest {
        digest_array(domain, &self.0)
    }

    fn value(&self) -> Value {
        let tuples = self
            .0
            .iter()
            .map(|bytes| serde_json::from_slice(bytes).expect("canonical bytes are JSON"));
        Value::Array(tuples.collect())
    }
}

/// A registry root view: the registry's name and its entries as name and definition, sorted by
/// name; entry keys are not content (RQ-61).
fn registry_view<'a>(
    name: &'a str,
    entries: impl Iterator<Item = [&'a str; 2]>,
) -> (&'a str, Vec<[&'a str; 2]>) {
    let mut rows: Vec<[&str; 2]> = entries.collect();
    rows.sort_unstable();
    (name, rows)
}

/// The binding root view: key, live label, aliases and computed hash, sorted by key (RQ-15,
/// AC-240).
fn bindings_view<'a>(
    document: &'a Document,
    resolve: &impl Fn(ElementId) -> Digest,
) -> Vec<(Key, Label, &'a [Label], Digest)> {
    document
        .bindings
        .iter()
        .map(|binding| {
            (
                binding.element.key(),
                binding.label,
                binding.aliases.as_slice(),
                resolve(binding.element),
            )
        })
        .collect()
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

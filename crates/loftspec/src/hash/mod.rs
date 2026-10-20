//! The hasher (CMP-3): content hashes, decision view hashes and the roots of a document
//! (F-3, F-4, F-5). Each view is RFC 8785 canonical JSON; each digest is domain-separated. The
//! hasher takes an indexed document, so every reference it follows resolves.

mod decisions;
mod state;
mod tree;

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};

pub use state::{StateView, lint_digest, state_root, state_view};
pub use tree::ContentHash;

use crate::canonical::{Digest, Domain, canonical_bytes, digest, digest_array};
use crate::decision::Decision;
use crate::document::{Document, Edge, EdgeTarget, SubjectRole};
use crate::ext::Ext;
use crate::identity::{Key, Label};
use crate::ids::{DecisionId, ElementId, NodeId};
use crate::index::{IndexErrors, Indexed};

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
    /// The term list root; none for a document whose term list is empty.
    pub terms: Option<Digest>,
    pub decisions: Digest,
    pub content: Digest,
    pub bindings: Digest,
    /// The extensions root; none for a document in which no node carries `ext`.
    pub extensions: Option<Digest>,
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

/// The content view of every keyed node of a document without the hashes of its children: what
/// the node itself says. The document need not index, since no view follows a reference.
pub(crate) fn own_views(document: &Document) -> BTreeMap<NodeId, Value> {
    let mut views = BTreeMap::new();
    hash_trees(document, Some(&mut views));
    for view in views.values_mut() {
        if let Value::Array(fields) = view {
            fields.pop();
        }
    }
    views
}

/// Indexes a document, then computes every content hash, decision view hash and root.
///
/// # Errors
///
/// Every integrity error the document's index finds.
pub fn hash_document(document: &Document) -> Result<DocumentHashes, IndexErrors> {
    Ok(hash_indexed(&document.index()?))
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

/// The decision history view of every decision of an indexed document: its decision view with
/// each node and successor it names given by key.
#[must_use]
pub fn decision_history_views(indexed: &Indexed<'_>) -> BTreeMap<DecisionId, Value> {
    let decisions = &indexed.document().decisions;
    decisions
        .iter()
        .map(|decision| (decision.id, decisions::history_view(decision)))
        .collect()
}

/// The decision history hash of every decision of an indexed document: the digest of its
/// decision history view under `loftspec/decision-history/1`. No root holds it.
#[must_use]
pub fn decision_history_hashes(indexed: &Indexed<'_>) -> BTreeMap<DecisionId, Digest> {
    let decisions = &indexed.document().decisions;
    decisions
        .iter()
        .map(|decision| (decision.id, decision_history_hash(decision)))
        .collect()
}

/// The decision history hash of one decision. Its view names every node and decision by key and
/// follows no reference, so the decision need not belong to a document that indexes.
pub(crate) fn decision_history_hash(decision: &Decision) -> Digest {
    digest(Domain::DecisionHistory, &decisions::history_view(decision))
}

/// Digests a root view, keeping it as a value when views are kept.
fn root<T: Serialize>(
    views: &mut Option<&mut Views>,
    name: &'static str,
    domain: Domain,
    view: &T,
) -> Digest {
    if let Some(views) = views {
        let value = serde_json::to_value(view).expect("a root view serialises");
        views.roots.push((name, domain, value));
    }
    digest(domain, view)
}

fn hash(indexed: &Indexed<'_>, mut views: Option<&mut Views>) -> DocumentHashes {
    let document = indexed.document();
    let trees = hash_trees(document, views.as_deref_mut().map(|views| &mut views.nodes));
    let index = trees.index();
    let (decisions, decision_views) =
        decisions::DecisionHasher::new(&document.decisions, &index).hash_all();
    let resolve = |element: ElementId| match element {
        ElementId::Node(id) => node_hash(&index, id),
        ElementId::Decision(id) => decisions[&id],
    };
    let edges = edges_root(&document.edges, &resolve, &mut views);
    let tag_rows = document
        .tags
        .iter()
        .map(|tag| [tag.name.as_str(), tag.definition.as_str()]);
    let tags = root(
        &mut views,
        "tags",
        Domain::Registry,
        &registry_view("tags", tag_rows),
    );
    let category_rows = document
        .categories
        .iter()
        .map(|category| [category.name.as_str(), category.definition.as_str()]);
    let categories = root(
        &mut views,
        "categories",
        Domain::Registry,
        &registry_view("categories", category_rows),
    );
    let mut decision_hashes: Vec<Digest> = decisions.values().copied().collect();
    decision_hashes.sort_unstable();
    let decisions_root = root(&mut views, "decisions", Domain::Decisions, &decision_hashes);
    let terms = (!document.terms.is_empty())
        .then(|| root(&mut views, "terms", Domain::Registry, &terms_view(document)));
    let mut content_view = vec![
        json!(trees.products),
        json!(trees.components),
        json!(edges),
        json!(tags),
        json!(categories),
        json!(decisions_root),
    ];
    content_view.extend(terms.map(|terms| json!(terms)));
    let content = root(&mut views, "content", Domain::Content, &content_view);
    let bindings = root(
        &mut views,
        "bindings",
        Domain::Bindings,
        &bindings_view(document, &resolve),
    );
    let extended = extensions_view(document);
    let extensions = (!extended.is_empty())
        .then(|| root(&mut views, "extensions", Domain::Extensions, &extended));
    let state = root(
        &mut views,
        "state",
        Domain::State,
        &state::view_one(document, content, bindings, extensions),
    );
    if let Some(views) = views {
        views.decisions = decision_views;
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
            terms,
            decisions: decisions_root,
            content,
            bindings,
            extensions,
            state,
        },
    }
}

/// Each node that carries `ext`, as its key and its `ext`, sorted by key.
fn extensions_view(document: &Document) -> Vec<(Key, &Ext)> {
    let mut extended = Vec::new();
    document.walk(&mut |node| {
        if let Some(id) = node.id()
            && !node.ext().is_empty()
        {
            extended.push((id.key(), node.ext()));
        }
    });
    extended.sort_by_key(|(key, _)| *key);
    extended
}

/// How a view names a node the document no longer holds: `{"formerLabel": <label>}` (ADR-13).
pub(crate) fn former(label: Label) -> Value {
    json!({ "formerLabel": label })
}

/// The edges root: each edge as its source's hash, its type, its target's hash and its role,
/// the tuples sorted by their canonical bytes, duplicates kept (RQ-13). An endpoint's hash is a
/// node's content hash or a decision's view hash; a former label enters as itself. Each tuple is
/// canonicalised once, and the sorted bytes are digested as the array they make.
fn edges_root(
    edges: &[Edge],
    resolve: &impl Fn(ElementId) -> Digest,
    views: &mut Option<&mut Views>,
) -> Digest {
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
    if let Some(views) = views {
        let parse = |bytes: &Vec<u8>| -> Value {
            serde_json::from_slice(bytes).expect("canonical bytes are JSON")
        };
        let value = Value::Array(tuples.iter().map(parse).collect());
        views.roots.push(("edges", Domain::Edges, value));
    }
    digest_array(Domain::Edges, &tuples)
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

/// A term as its root view holds it: its name, its definition and its sorted aliases.
type TermRow<'a> = (&'a str, &'a str, Vec<&'a str>);

/// The term list root view: `terms` and each entry as its name, its definition and its aliases,
/// sorted by name, then definition, then aliases; entry keys are not content (RQ-61).
fn terms_view(document: &Document) -> (&'static str, Vec<TermRow<'_>>) {
    let mut rows: Vec<_> = document
        .terms
        .iter()
        .map(|term| {
            let mut aliases: Vec<&str> = term.aliases.iter().map(String::as_str).collect();
            aliases.sort_unstable();
            (term.name.as_str(), term.definition.as_str(), aliases)
        })
        .collect();
    rows.sort_unstable();
    ("terms", rows)
}

/// The binding root view: key, live label, aliases and computed hash, sorted by key (RQ-15,
/// AC-240). A departed binding names nothing, so it is left out.
fn bindings_view<'a>(
    document: &'a Document,
    resolve: &impl Fn(ElementId) -> Digest,
) -> Vec<(Key, Label, &'a [Label], Digest)> {
    document
        .bindings
        .live()
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

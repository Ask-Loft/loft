//! The comparer (Comparing two versions): two versions of one document, held in memory, matched
//! by key (the decision "Two versions are compared by key, with whole records"). Each node and
//! decision that differs is reported with its whole record in each version that holds it, as
//! the text form writes it; a node is told added, changed, moved and removed as
//! [`node_changes`](super::node_changes) tells it, by its record in place of its content view,
//! and reordered where the children both versions hold stand in another order. The comparer
//! judges nothing.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::{Keyed, NodeChange, Parents, change_of};
use crate::codec::{SpecPath, records};
use crate::conflict::ConflictOwner;
use crate::document::Document;
use crate::hash::registry_roots;
use crate::ids::{DecisionId, NodeId};
use crate::wire::wire_names;

/// One version of a document as the comparer is given it: the document, and the text of its
/// `spec/lint.yaml` where it holds one.
#[derive(Debug, Clone, Copy)]
pub struct Version<'a> {
    pub document: &'a Document,
    pub lint: Option<&'a str>,
}

/// Which of the two versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Older,
    Newer,
}

/// How a node differs between the versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difference {
    Added,
    Changed,
    Moved,
    Removed,
    Reordered,
}

/// A node as one version holds it: its parent, none for a Product or a root Component, and its
/// own record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Side {
    pub parent: Option<NodeId>,
    pub record: Value,
}

/// The children both versions hold, in the order of each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub older: Vec<NodeId>,
    pub newer: Vec<NodeId>,
}

/// A node that differs, as each version that holds it holds it, and the order of its children
/// where that differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDifference {
    pub id: NodeId,
    pub difference: Difference,
    pub older: Option<Side>,
    pub newer: Option<Side>,
    pub order: Option<Order>,
}

/// How a decision differs between the versions; a decision has no parent, so it never moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionChange {
    Added,
    Changed,
    Removed,
}

/// A decision that differs, with its record in each version that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionDifference {
    pub id: DecisionId,
    pub change: DecisionChange,
    pub older: Option<Value>,
    pub newer: Option<Value>,
}

/// A part of a version that the comparer says differs or not as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    /// The tag registry, by its root.
    Tags,
    /// The category registry, by its root.
    Categories,
    /// The term list, by its root; a version without `spec/terms.yaml` holds an empty list.
    Terms,
    /// The text of `spec/lint.yaml`, which a version may not hold.
    LintConfiguration,
    /// The counters and every field of every binding of `spec/labels.yaml`.
    Bindings,
}

wire_names!(Part {
    Tags => "tags",
    Categories => "categories",
    Terms => "terms",
    LintConfiguration => "lintConfiguration",
    Bindings => "bindings",
});

/// What differs between two versions: the nodes in the newer version's document order, then the
/// removed in the older's; the Products' order where it differs; the decisions by key; and the
/// parts that differ, in [`Part`]'s order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub nodes: Vec<NodeDifference>,
    pub products: Option<Order>,
    pub decisions: Vec<DecisionDifference>,
    pub parts: Vec<Part>,
}

impl Comparison {
    /// Whether `part` differs between the versions.
    #[must_use]
    pub fn differs(&self, part: Part) -> bool {
        self.parts.contains(&part)
    }
}

/// Why two versions cannot be compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A node without a key, in the file the text form writes it in.
    Unkeyed { version: Which, path: SpecPath },
    /// A conflict record, and the record that holds it.
    Conflict {
        version: Which,
        owner: ConflictOwner,
    },
}

/// Compares `older` with `newer`.
///
/// # Errors
///
/// Each node without a key, and each conflict record, of either version.
pub fn compare(older: Version<'_>, newer: Version<'_>) -> Result<Comparison, Vec<Refusal>> {
    let mut refused = Vec::new();
    let then = Read::of(older.document, Which::Older, &mut refused);
    let now = Read::of(newer.document, Which::Newer, &mut refused);
    if !refused.is_empty() {
        return Err(refused);
    }
    let (old, new) = (older.document, newer.document);
    let (old_tags, old_categories, old_terms) = registry_roots(old);
    let (new_tags, new_categories, new_terms) = registry_roots(new);
    let parts = [
        (Part::Tags, old_tags != new_tags),
        (Part::Categories, old_categories != new_categories),
        (Part::Terms, old_terms != new_terms),
        (Part::LintConfiguration, older.lint != newer.lint),
        (
            Part::Bindings,
            old.bindings != new.bindings || old.counters != new.counters,
        ),
    ];
    Ok(Comparison {
        nodes: nodes(&then, &now),
        products: reordering(&products(old), &products(new)),
        decisions: decisions(&then.decisions, &now.decisions),
        parts: parts
            .into_iter()
            .filter_map(|(part, differs)| differs.then_some(part))
            .collect(),
    })
}

/// What the comparer reads of one version: each node's record and its order, each decision's
/// record, and each node's parent and children.
struct Read {
    keyed: Keyed,
    decisions: BTreeMap<DecisionId, Value>,
    parents: Parents,
    children: BTreeMap<NodeId, Vec<NodeId>>,
}

impl Read {
    fn of(document: &Document, which: Which, refused: &mut Vec<Refusal>) -> Self {
        let records = records(document);
        refused.extend(records.unkeyed.into_iter().map(|path| Refusal::Unkeyed {
            version: which,
            path,
        }));
        refused.extend(document.conflicts.iter().map(|conflict| Refusal::Conflict {
            version: which,
            owner: conflict.owner,
        }));
        let mut children: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        document.walk(&mut |node| {
            if let Some(id) = node.id() {
                let keyed = node
                    .children()
                    .iter()
                    .filter_map(|child| child.id())
                    .collect();
                children.insert(id, keyed);
            }
        });
        // A version compared holds no node without a key, so each node's children are keyed.
        let parents = (children.iter())
            .flat_map(|(parent, ids)| ids.iter().map(|child| (*child, *parent)))
            .collect();
        Self {
            keyed: Keyed {
                views: records.nodes,
                order: Keyed::order_of(document),
            },
            decisions: records.decisions,
            parents,
            children,
        }
    }

    fn side(&self, id: NodeId) -> Option<Side> {
        let record = self.keyed.views.get(&id)?.clone();
        Some(Side {
            parent: self.parents.get(&id).copied(),
            record,
        })
    }
}

fn nodes(then: &Read, now: &Read) -> Vec<NodeDifference> {
    let parents = Some((&then.parents, &now.parents));
    let mut nodes = Vec::new();
    for id in &now.keyed.order {
        let order = then
            .children
            .get(id)
            .and_then(|older| reordering(older, &now.children[id]));
        let difference = match (change_of(&then.keyed, &now.keyed, parents, *id), &order) {
            (Some(NodeChange::Added), _) => Difference::Added,
            (Some(NodeChange::Changed), _) => Difference::Changed,
            (Some(NodeChange::Moved), _) => Difference::Moved,
            (None, Some(_)) => Difference::Reordered,
            (None, None) => continue,
        };
        nodes.push(NodeDifference {
            id: *id,
            difference,
            older: then.side(*id),
            newer: now.side(*id),
            order,
        });
    }
    let removed = then
        .keyed
        .order
        .iter()
        .filter(|id| !now.keyed.views.contains_key(id));
    nodes.extend(removed.map(|id| NodeDifference {
        id: *id,
        difference: Difference::Removed,
        older: then.side(*id),
        newer: None,
        order: None,
    }));
    nodes
}

/// The order of the IDs both lists hold, where it differs between them.
fn reordering(older: &[NodeId], newer: &[NodeId]) -> Option<Order> {
    let shared = |of: &[NodeId], other: &[NodeId]| {
        let other: BTreeSet<_> = other.iter().collect();
        of.iter()
            .filter(|id| other.contains(id))
            .copied()
            .collect::<Vec<_>>()
    };
    let found = Order {
        older: shared(older, newer),
        newer: shared(newer, older),
    };
    (found.older != found.newer).then_some(found)
}

fn products(document: &Document) -> Vec<NodeId> {
    let ids = document.products.iter().filter_map(|product| product.id);
    ids.map(NodeId::Product).collect()
}

fn decisions(
    then: &BTreeMap<DecisionId, Value>,
    now: &BTreeMap<DecisionId, Value>,
) -> Vec<DecisionDifference> {
    let ids: BTreeSet<DecisionId> = then.keys().chain(now.keys()).copied().collect();
    ids.into_iter()
        .filter_map(|id| {
            let (older, newer) = (then.get(&id), now.get(&id));
            let change = match (older, newer) {
                (None, Some(_)) => DecisionChange::Added,
                (Some(_), None) => DecisionChange::Removed,
                (Some(a), Some(b)) if a != b => DecisionChange::Changed,
                _ => return None,
            };
            Some(DecisionDifference {
                id,
                change,
                older: older.cloned(),
                newer: newer.cloned(),
            })
        })
        .collect()
}

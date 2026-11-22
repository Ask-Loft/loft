//! The edges a `spec-change` view's diagram draws between its boxes (the See a spec change
//! Feature): `realizes`, `provides` and `requires` as the tree holds them, and as the base did
//! where the branch removed one of their ends, and each drawn decision's subject edges.

use std::collections::BTreeSet;

use loftspec::bindings::Bindings;
use loftspec::decision::NodeRef;
use loftspec::document::{Edge, EdgeKind, EdgeTarget};
use loftspec::identity::Key;
use serde_json::{Value, json};

use super::graph::{Decided, Documents};

/// The `realizes`, `provides` and `requires` edges between boxes drawn, as the tree holds them,
/// and as the base did where the branch removed one of their ends; and the subject edges from each decision drawn
/// to the box of each node it names, where that box is drawn: an added or changed decision's as
/// the tree holds them, and a removed decision's as the base did.
pub(super) fn edges(shown: &[Key], decided: &[Decided], documents: &Documents<'_>) -> Value {
    let Documents {
        base,
        document,
        now,
        ..
    } = *documents;
    let drawn: BTreeSet<Key> = shown.iter().copied().collect();
    let held = |removed: bool| -> BTreeSet<Key> {
        decided
            .iter()
            .filter(|decision| decision.removed == removed)
            .map(|decision| decision.key)
            .collect()
    };
    let (in_tree, in_base) = (held(false), held(true));
    // A base edge between two nodes the tree holds is the tree's to draw, or not.
    let both_held = |edge: &&Edge| {
        linked(edge).is_some_and(|(from, to, _)| [from, to].iter().all(|key| now.holds(key)))
    };
    let tree = (document.edges.iter())
        .filter_map(|edge| drawn_edge(edge, &drawn, &in_tree, &document.bindings));
    let gone = (base.edges.iter())
        .filter(|edge| !both_held(edge))
        .filter_map(|edge| drawn_edge(edge, &drawn, &in_base, &base.bindings));
    let found: BTreeSet<Drawn> = tree.chain(gone).collect();
    let edges = found.into_iter().map(|(from, to, kind, role)| {
        let mut edge = json!({ "from": from, "to": to, "kind": kind });
        if let Some(role) = role {
            edge["role"] = json!(role);
        }
        edge
    });
    Value::Array(edges.collect())
}

/// An edge as the diagram draws it: from, to, its kind and a subject edge's role.
type Drawn = (String, String, &'static str, Option<&'static str>);

/// The ends and kind of a `realizes`, `provides` or `requires` edge, which joins two nodes.
fn linked(edge: &Edge) -> Option<(Key, Key, &'static str)> {
    let kind = edge.kind();
    if !matches!(
        kind,
        EdgeKind::Realizes | EdgeKind::Provides | EdgeKind::Requires
    ) {
        return None;
    }
    let EdgeTarget::Element(target) = edge.target() else {
        return None;
    };
    Some((edge.source().key(), target.key(), kind.as_str()))
}

/// `edge` as the diagram draws it, where both its ends are drawn: a `realizes`, `provides` or
/// `requires` edge between two boxes, or a subject edge from a decision drawn to the box of the
/// node it names.
fn drawn_edge(
    edge: &Edge,
    drawn: &BTreeSet<Key>,
    named: &BTreeSet<Key>,
    bindings: &Bindings,
) -> Option<Drawn> {
    if let Some((from, to, kind)) = linked(edge) {
        return (drawn.contains(&from) && drawn.contains(&to))
            .then(|| (from.to_string(), to.to_string(), kind, None));
    }
    match *edge {
        Edge::Subject {
            decision,
            subject,
            role,
        } if named.contains(&decision.key()) => {
            // A node named by its former label is the node the label's binding gives.
            let NodeRef::Held(node) = bindings.node(subject) else {
                return None;
            };
            let (from, to) = (decision.key(), node.key());
            drawn.contains(&to).then(|| {
                (
                    from.to_string(),
                    to.to_string(),
                    "subject",
                    Some(role.as_str()),
                )
            })
        }
        _ => None,
    }
}

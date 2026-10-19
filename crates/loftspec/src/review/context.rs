//! The scope of each assertion, and the nodes a unit gives around it: the nodes above the scope,
//! what the scope holds, a Feature's Requirements and the Components it realizes.

use std::collections::BTreeMap;

use crate::document::Edge;
use crate::ids::NodeId;
use crate::kind::NodeKind;
use crate::node::Node;

use super::links;
use super::side::{Pair, Side, Which};
use super::{Assertion, Fact, Holder, Realized, Unit, WithCriteria};

/// A scope and the side that holds it: the tree wherever it holds the node.
type Scope = (Which, NodeId);

/// A scope and the assertions it holds.
type Grouped = (Scope, Vec<Assertion>);

/// The scope of the assertion `id`: the Feature above a Requirement or a criterion, the node
/// that holds a Constraint or a Quality Attribute.
fn scope_of(side: &Side<'_>, id: NodeId) -> NodeId {
    let ancestors = side.ancestors(id);
    let found = match id.kind() {
        NodeKind::Requirement | NodeKind::Criterion => ancestors
            .iter()
            .find(|above| above.kind() == NodeKind::Feature)
            .copied(),
        _ => ancestors.first().copied(),
    };
    found.expect("an assertion lies below its scope")
}

/// The assertions grouped by scope, the tree's scopes in its order, then those only the base
/// holds in the base's.
pub(super) fn group(sides: &Pair<'_>, reviewed: Vec<(Which, Assertion)>) -> Vec<Grouped> {
    let position = |which: Which, id: NodeId| {
        let order = &sides.side(which).order;
        let at = order
            .iter()
            .position(|each| *each == id)
            .unwrap_or(order.len());
        (which == Which::Base, at)
    };
    let mut units: BTreeMap<(bool, usize), Grouped> = BTreeMap::new();
    for (which, assertion) in reviewed {
        let scope = scope_of(sides.side(which), assertion.id);
        let holder = if sides.tree.holds(scope) {
            Which::Tree
        } else {
            which
        };
        units
            .entry(position(holder, scope))
            .or_insert_with(|| ((holder, scope), Vec::new()))
            .1
            .push(assertion);
    }
    units.into_values().collect()
}

/// The facts of the keyed nodes among `nodes`.
fn facts<'a>(side: &Side<'a>, nodes: impl IntoIterator<Item = Node<'a>>) -> Vec<Fact> {
    nodes
        .into_iter()
        .filter_map(Node::id)
        .map(|id| side.fact(id))
        .collect()
}

/// The node `id` with the Constraints and Quality Attributes it holds.
fn holder(side: &Side<'_>, id: NodeId) -> Holder {
    let children = side.node(id).children();
    let of = |kind: NodeKind| {
        let nodes = children
            .iter()
            .copied()
            .filter(|child| child.kind() == kind);
        facts(side, nodes)
    };
    Holder {
        node: side.fact(id),
        constraints: of(NodeKind::Constraint),
        quality_attributes: of(NodeKind::QualityAttribute),
    }
}

/// The holders above `id`, the root first.
fn above(side: &Side<'_>, id: NodeId) -> Vec<Holder> {
    let mut ancestors = side.ancestors(id);
    ancestors.reverse();
    ancestors
        .into_iter()
        .map(|each| holder(side, each))
        .collect()
}

/// Every Requirement of the Feature `scope`, with its criteria.
fn requirements(side: &Side<'_>, scope: NodeId) -> Vec<WithCriteria> {
    let Node::Feature(feature) = side.node(scope) else {
        return Vec::new();
    };
    feature
        .requirements
        .iter()
        .filter_map(|requirement| {
            let id = requirement.id?.into();
            let criteria = facts(side, requirement.criteria.iter().map(Node::Criterion));
            Some(WithCriteria {
                requirement: side.fact(id),
                criteria,
            })
        })
        .collect()
}

/// Each Component the Feature `scope` realizes, with the Components above it.
fn realizes(side: &Side<'_>, scope: NodeId) -> Vec<Realized> {
    side.document
        .edges
        .iter()
        .filter_map(|edge| match edge {
            Edge::Realizes { feature, component } if NodeId::from(*feature) == scope => {
                Some(NodeId::from(*component))
            }
            _ => None,
        })
        .filter(|component| side.holds(*component))
        .map(|component| Realized {
            component: holder(side, component),
            above: above(side, component),
        })
        .collect()
}

/// The unit of `scope`, reviewing `assertions`.
pub(super) fn unit(sides: &Pair<'_>, (which, scope): Scope, assertions: Vec<Assertion>) -> Unit {
    let side = sides.side(which);
    let mut bearing: Vec<NodeId> = assertions.iter().map(|assertion| assertion.id).collect();
    bearing.push(scope);
    let prose = assertions
        .iter()
        .map(|assertion| sides.node_anywhere(assertion.id));
    Unit {
        scope: holder(side, scope),
        ancestors: above(side, scope),
        requirements: requirements(side, scope),
        realizes: realizes(side, scope),
        decisions: links::decisions(side.document, &bearing),
        terms: links::terms(&sides.tree.document.terms, prose),
        assertions,
    }
}

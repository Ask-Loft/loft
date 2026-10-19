//! The scope of each assertion, and the nodes a unit gives around it: the nodes above the scope,
//! what the scope holds, a Feature's Requirements and the Components it realizes.

use std::collections::BTreeMap;

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
/// holds in the base's. The side a scope is placed by always holds it: the tree where it does,
/// and otherwise the base, whose assertion it lies above.
pub(super) fn group(sides: &Pair<'_>, reviewed: Vec<(Which, Assertion)>) -> Vec<Grouped> {
    let mut units: BTreeMap<(bool, usize), Grouped> = BTreeMap::new();
    for (which, assertion) in reviewed {
        let scope = scope_of(sides.side(which), assertion.id);
        let holder = if sides.tree.holds(scope) {
            Which::Tree
        } else {
            which
        };
        let place = (holder == Which::Base, sides.side(holder).ordinal(scope));
        units
            .entry(place)
            .or_insert_with(|| ((holder, scope), Vec::new()))
            .1
            .push(assertion);
    }
    units.into_values().collect()
}

/// The holders above `id`, the root first.
fn above(side: &Side<'_>, id: NodeId) -> Vec<Holder> {
    let mut ancestors = side.ancestors(id);
    ancestors.reverse();
    ancestors
        .into_iter()
        .map(|each| side.holder(each))
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
            let criteria: Vec<Fact> = requirement
                .criteria
                .iter()
                .filter_map(|criterion| Node::Criterion(criterion).id())
                .map(|criterion| side.fact(criterion))
                .collect();
            Some(WithCriteria {
                requirement: side.fact(id),
                criteria,
            })
        })
        .collect()
}

/// Each Component the Feature `scope` realizes, with the Components above it.
fn realizes(side: &Side<'_>, scope: NodeId) -> Vec<Realized> {
    side.realized(scope)
        .map(|component| Realized {
            component: side.holder(component),
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
        scope: side.holder(scope),
        ancestors: above(side, scope),
        requirements: requirements(side, scope),
        realizes: realizes(side, scope),
        decisions: links::decisions(side.document, &side.links, &bearing),
        terms: links::terms(&sides.phrases, prose),
        assertions,
    }
}

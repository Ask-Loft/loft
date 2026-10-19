//! The review packet (the `loft review` Feature): the assertions under review, each in the unit
//! of its scope with the nodes it is read against, and nothing that weighs them (the decision
//! "The review packet carries context and no judgement"). An assertion is a Requirement, a
//! criterion, a Constraint or a Quality Attribute. Without a base every assertion is reviewed;
//! against a base, those the tree added, changed, moved or removed, each beside the base's node.

mod context;
mod links;
mod side;

use serde_json::Value;

use crate::decision::StateKind;
use crate::document::{Document, TermEntry};
use crate::identity::Label;
use crate::ids::{DecisionId, NodeId};
use crate::kind::NodeKind;

use self::side::{Side, Which};

/// How an assertion, or a node beside it, differs from the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added,
    Changed,
    Moved,
    Removed,
}

impl Change {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Moved => "moved",
            Self::Removed => "removed",
        }
    }
}

/// A node as the packet gives it, from the tree or, where only the base holds it, the base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: Option<Label>,
    pub name: String,
    /// The node's own content view, its children's hashes left out.
    pub view: Value,
    pub change: Option<Change>,
    /// Whether the fact is read from the base, which alone holds the node.
    pub from_base: bool,
}

/// A node with the Constraints and Quality Attributes it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub node: Fact,
    pub constraints: Vec<Fact>,
    pub quality_attributes: Vec<Fact>,
}

/// A Requirement with its criteria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithCriteria {
    pub requirement: Fact,
    pub criteria: Vec<Fact>,
}

/// A Component a Feature realizes, with the Components above it, nearest last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Realized {
    pub component: Holder,
    pub above: Vec<Holder>,
}

/// An assertion under review: its change against the base, and the base's node where the
/// assertion changed, moved or was removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assertion {
    pub id: NodeId,
    pub kind: NodeKind,
    pub change: Option<Change>,
    pub before: Option<Fact>,
}

/// How a decision links to an assertion or a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Via {
    SubjectOutcome,
    SubjectAffected,
    Driver,
    Consequence,
    Mentions,
}

impl Via {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SubjectOutcome => "subject:outcome",
            Self::SubjectAffected => "subject:affected",
            Self::Driver => "driver",
            Self::Consequence => "consequence",
            Self::Mentions => "mentions",
        }
    }
}

/// A decision that bears on a unit, and each node of the unit it links to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linked {
    pub id: DecisionId,
    pub label: Option<Label>,
    pub title: String,
    pub state: StateKind,
    pub outcome: Option<String>,
    pub links: Vec<(NodeId, Via)>,
}

/// The assertions of one scope and what they are read against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub scope: Holder,
    /// The nodes above the scope, the root first.
    pub ancestors: Vec<Holder>,
    /// For a Feature, every Requirement it holds.
    pub requirements: Vec<WithCriteria>,
    /// For a Feature, every Component it realizes.
    pub realizes: Vec<Realized>,
    pub assertions: Vec<Assertion>,
    pub decisions: Vec<Linked>,
    pub terms: Vec<TermEntry>,
}

/// The units of a review, in the order of their scopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub units: Vec<Unit>,
}

/// A node the tree holds without a key, which no review can follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unkeyed {
    pub kind: NodeKind,
    pub name: String,
}

/// The kinds of node a review reads as assertions.
const fn is_assertion(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Requirement
            | NodeKind::Criterion
            | NodeKind::Constraint
            | NodeKind::QualityAttribute
    )
}

/// The review packet of `document`: against `base` where one is given, and only at or below
/// `node` where one is.
///
/// # Errors
///
/// Each node of `document` that has no key.
pub fn pack(
    document: &Document,
    base: Option<&Document>,
    node: Option<NodeId>,
) -> Result<Packet, Vec<Unkeyed>> {
    let unkeyed = unkeyed(document);
    if !unkeyed.is_empty() {
        return Err(unkeyed);
    }
    let sides = Side::pair(document, base);
    let reviewed = reviewed(&sides, node);
    let units = context::group(&sides, reviewed)
        .into_iter()
        .map(|(scope, assertions)| context::unit(&sides, scope, assertions))
        .collect();
    Ok(Packet { units })
}

/// Every node of `document` without a key, in document order.
fn unkeyed(document: &Document) -> Vec<Unkeyed> {
    let mut found = Vec::new();
    document.walk(&mut |node| {
        if node.id().is_none() {
            found.push(Unkeyed {
                kind: node.kind(),
                name: node.name().into_owned(),
            });
        }
    });
    found
}

/// The assertions under review, the tree's in its order and then the removed in the base's,
/// each with the side that holds it.
fn reviewed(sides: &side::Pair<'_>, node: Option<NodeId>) -> Vec<(Which, Assertion)> {
    let tree = &sides.tree;
    let mut reviewed: Vec<(Which, Assertion)> = tree
        .order
        .iter()
        .filter(|id| is_assertion(id.kind()))
        .filter(|id| sides.base.is_none() || tree.change(**id).is_some())
        .map(|id| (Which::Tree, sides.assertion(*id, Which::Tree)))
        .collect();
    if let Some(base) = &sides.base {
        reviewed.extend(
            base.order
                .iter()
                .filter(|id| is_assertion(id.kind()) && !tree.holds(**id))
                .map(|id| (Which::Base, sides.assertion(*id, Which::Base))),
        );
    }
    reviewed.retain(|(which, assertion)| {
        node.is_none_or(|node| sides.side(*which).within(assertion.id, node))
    });
    reviewed
}

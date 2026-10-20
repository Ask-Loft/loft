//! The index of a document (CMP-2): every key the document carries, which element holds it, and
//! where each node sits. Building it finds every integrity error at once: a key carried twice, an
//! edge, binding or decision reference that names nothing the document holds, and a successor
//! that is missing or leads back to itself. A document whose index builds cleanly can be hashed
//! without error.

use std::collections::{BTreeMap, BTreeSet};

use crate::decision::{Decision, DecisionState, DriverPayload, NodeRef};
use crate::document::{Document, EdgeTarget};
use crate::identity::Key;
use crate::ids::{DecisionId, ElementId, NodeId, OptionId};
use crate::node::Node;

/// A document whose references all resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IntegrityError {
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
    #[error(
        "decision {decision} names {successor} as its successor, which the document does not hold"
    )]
    UnknownSuccessor {
        decision: DecisionId,
        successor: DecisionId,
    },
    #[error("decision {0} names itself as its successor")]
    SupersedesItself(DecisionId),
}

/// Superseded decisions whose successors lead back to themselves, each named once, in key
/// order (RQ-183).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("decisions {} supersede one another in a cycle", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "))]
pub struct SupersessionCycle(pub Vec<DecisionId>);

/// Every key of a document and the element that carries it, with each node's parent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    elements: BTreeMap<Key, ElementId>,
    parents: BTreeMap<NodeId, NodeId>,
}

impl Index {
    /// The element that carries `key`, if a node or decision does.
    #[must_use]
    pub fn element(&self, key: Key) -> Option<ElementId> {
        self.elements.get(&key).copied()
    }

    /// Whether the document holds `element`: its key, carried by an element of that kind.
    #[must_use]
    pub fn holds(&self, element: ElementId) -> bool {
        self.element(element.key()) == Some(element)
    }

    /// The node that holds `node`; none for a Product or a root Component.
    #[must_use]
    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.parents.get(&node).copied()
    }

    /// The nodes above `node`, nearest first.
    pub fn ancestors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(self.parent(node), |above| self.parent(*above))
    }

    /// Indexes a document, collecting every integrity error rather than stopping at the first.
    #[must_use]
    pub fn build(document: &Document) -> (Self, Vec<IntegrityError>, Vec<SupersessionCycle>) {
        let mut builder = Builder::default();
        builder.nodes(document);
        builder.decisions(&document.decisions);
        builder.registries(document);
        let Builder {
            index, mut errors, ..
        } = builder;
        errors.extend(references(document, &index));
        let cycles = supersession(&document.decisions, &mut errors);
        (index, errors, cycles)
    }
}

/// A document together with its clean index: what the hasher takes, so that hashing cannot meet
/// an unresolved reference.
#[derive(Debug, Clone)]
pub struct Indexed<'a> {
    document: &'a Document,
    index: Index,
}

impl<'a> Indexed<'a> {
    #[must_use]
    pub const fn document(&self) -> &'a Document {
        self.document
    }

    #[must_use]
    pub const fn index(&self) -> &Index {
        &self.index
    }
}

/// Every reason a document cannot be indexed cleanly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexErrors {
    pub integrity: Vec<IntegrityError>,
    pub cycles: Vec<SupersessionCycle>,
}

impl Document {
    /// The document with its index, or every integrity error it holds.
    ///
    /// # Errors
    ///
    /// Every key carried twice, every reference that names nothing the document holds, and every
    /// supersession cycle.
    pub fn index(&self) -> Result<Indexed<'_>, IndexErrors> {
        let (index, integrity, cycles) = Index::build(self);
        if integrity.is_empty() && cycles.is_empty() {
            Ok(Indexed {
                document: self,
                index,
            })
        } else {
            Err(IndexErrors { integrity, cycles })
        }
    }
}

#[derive(Default)]
struct Builder {
    index: Index,
    /// Every key seen, of elements and of decision elements and registry entries alike.
    seen: BTreeSet<Key>,
    errors: Vec<IntegrityError>,
}

impl Builder {
    fn carry(&mut self, key: Key) {
        if !self.seen.insert(key) {
            self.errors.push(IntegrityError::DuplicateKey(key));
        }
    }

    fn nodes(&mut self, document: &Document) {
        for root in document.root_nodes() {
            self.node(root, None);
        }
    }

    fn node(&mut self, node: Node<'_>, parent: Option<NodeId>) {
        let id = node.id();
        if let Some(id) = id {
            self.carry(id.key());
            self.index.elements.insert(id.key(), id.into());
            if let Some(parent) = parent {
                self.index.parents.insert(id, parent);
            }
        }
        for child in node.children() {
            self.node(child, id.or(parent));
        }
    }

    fn decisions(&mut self, decisions: &[Decision]) {
        for decision in decisions {
            self.carry(decision.id.key());
            self.index
                .elements
                .insert(decision.id.key(), decision.id.into());
            let elements = decision.drivers.iter().map(|driver| driver.id.key());
            let elements = elements
                .chain(decision.options.iter().map(|option| option.id.key()))
                .chain(decision.consequences.iter().map(|item| item.id.key()));
            for key in elements {
                self.carry(key);
            }
        }
    }

    fn registries(&mut self, document: &Document) {
        let tags = document.tags.iter().map(|tag| tag.id.key());
        let categories = document.categories.iter().map(|entry| entry.id.key());
        let terms = document.terms.iter().map(|entry| entry.id.key());
        for key in tags.chain(categories).chain(terms) {
            self.carry(key);
        }
    }
}

/// Every edge endpoint, binding and decision reference that names nothing the document holds.
fn references(document: &Document, index: &Index) -> Vec<IntegrityError> {
    let mut errors = Vec::new();
    for edge in &document.edges {
        let target = match edge.target() {
            EdgeTarget::Element(element) => Some(element),
            EdgeTarget::Former(_) => None,
        };
        for element in std::iter::once(edge.source()).chain(target) {
            if !index.holds(element) {
                errors.push(IntegrityError::UnknownEdgeEndpoint(element));
            }
        }
    }
    for binding in document.bindings.live() {
        if !index.holds(binding.element) {
            errors.push(IntegrityError::UnknownBindingElement(binding.element));
        }
    }
    for decision in &document.decisions {
        errors.extend(decision_references(decision, index).map(|reference| {
            IntegrityError::UnknownDecisionReference {
                decision: decision.id,
                reference,
            }
        }));
    }
    errors
}

/// The keys a decision names that are not its own options and drivers, or nodes the document
/// holds (RQ-181).
fn decision_references<'a>(
    decision: &'a Decision,
    index: &'a Index,
) -> impl Iterator<Item = Key> + 'a {
    let options: BTreeSet<Key> = decision.options.iter().map(|o| o.id.key()).collect();
    let drivers: BTreeSet<Key> = decision.drivers.iter().map(|d| d.id.key()).collect();
    let selected = selected_option(&decision.state).map(OptionId::key);
    let unknown_options = selected
        .into_iter()
        .chain(decision.assessments.iter().map(|a| a.option.key()))
        .filter(move |key| !options.contains(key));
    let unknown_drivers = decision
        .assessments
        .iter()
        .map(|assessment| assessment.driver.key())
        .filter(move |key| !drivers.contains(key));
    let subjects = decision
        .drivers
        .iter()
        .filter_map(|driver| match driver.payload {
            DriverPayload::Subject { subject } => Some(subject),
            DriverPayload::Local { .. } => None,
        })
        .chain(decision.consequences.iter().filter_map(|c| c.subject));
    let unknown_nodes = subjects.filter_map(move |subject| match subject {
        NodeRef::Held(node) if !index.holds(node.into()) => Some(node.key()),
        NodeRef::Held(_) | NodeRef::Former(_) => None,
    });
    unknown_options.chain(unknown_drivers).chain(unknown_nodes)
}

/// The option a decision's state selects, if it selects one.
pub(crate) const fn selected_option(state: &DecisionState) -> Option<OptionId> {
    match state {
        DecisionState::Accepted {
            selected_option, ..
        }
        | DecisionState::Superseded {
            selected_option, ..
        }
        | DecisionState::Retired {
            selected_option, ..
        } => Some(*selected_option),
        DecisionState::Proposed | DecisionState::Rejected { .. } => None,
    }
}

/// The successor a decision's state names, if it is superseded.
pub(crate) const fn successor(state: &DecisionState) -> Option<DecisionId> {
    match state {
        DecisionState::Superseded { successor, .. } => Some(*successor),
        _ => None,
    }
}

/// Checks every successor: it must be another decision of the document (RQ-182), and following
/// successors must never return to where it started (RQ-183). Each cycle is reported once.
fn supersession(
    decisions: &[Decision],
    errors: &mut Vec<IntegrityError>,
) -> Vec<SupersessionCycle> {
    let successors: BTreeMap<DecisionId, Option<DecisionId>> = decisions
        .iter()
        .map(|decision| (decision.id, successor(&decision.state)))
        .collect();
    for (&decision, &next) in &successors {
        match next {
            Some(next) if next == decision => {
                errors.push(IntegrityError::SupersedesItself(decision));
            }
            Some(next) if !successors.contains_key(&next) => {
                errors.push(IntegrityError::UnknownSuccessor {
                    decision,
                    successor: next,
                });
            }
            _ => {}
        }
    }
    let mut cycles = BTreeSet::new();
    let mut finished = BTreeSet::new();
    for &start in successors.keys() {
        let (path, cycle) = follow(start, &successors, &finished);
        cycles.extend(cycle);
        finished.extend(path);
    }
    cycles.into_iter().map(SupersessionCycle).collect()
}

/// Follows successors from `start` until a decision that is not superseded, one already
/// followed, or one already on the path. Returns the path and, when the path closed on itself
/// through more than one decision, that cycle in key order.
fn follow(
    start: DecisionId,
    successors: &BTreeMap<DecisionId, Option<DecisionId>>,
    finished: &BTreeSet<DecisionId>,
) -> (Vec<DecisionId>, Option<Vec<DecisionId>>) {
    let mut path = Vec::new();
    let mut current = Some(start);
    while let Some(decision) = current.filter(|decision| !finished.contains(decision)) {
        if let Some(at) = path.iter().position(|seen| *seen == decision) {
            let mut cycle = path[at..].to_vec();
            cycle.sort_unstable();
            return (path, (cycle.len() > 1).then_some(cycle));
        }
        path.push(decision);
        current = successors.get(&decision).copied().flatten();
    }
    (path, None)
}

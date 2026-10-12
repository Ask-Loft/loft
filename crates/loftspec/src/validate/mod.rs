//! The validator (CMP-6, D-11 FG-5): the rules that decide whether a document is valid, every
//! error reported rather than only the first (AC-523). It reads the document and changes nothing
//! (CON-8). Containment and the kinds each edge type joins are in the model's types and the
//! reader's schema, so a document can break them only as text, where the reader refuses it; the
//! validator checks the rules a typed document can still break.

mod decisions;
mod nodes;

use crate::bindings::BindingError;
use crate::document::Document;
use crate::identity::{Key, Label};
use crate::ids::{DecisionId, DriverId, ElementId, NodeId, OptionId};
use crate::index::{Index, IntegrityError, SupersessionCycle};
use crate::names::TagName;

/// A rule a document breaks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// A key carried twice, or a reference that names nothing the document holds (RQ-76,
    /// RQ-78, RQ-181, RQ-182).
    #[error(transparent)]
    Integrity(#[from] IntegrityError),
    /// Successors that lead back to where they started (RQ-183).
    #[error(transparent)]
    Cycle(#[from] SupersessionCycle),
    /// A live label under two keys, or an alias that collides (RQ-70, RQ-10, RQ-66).
    #[error(transparent)]
    Binding(#[from] BindingError),
    /// A node that lists one tag twice (RQ-79).
    #[error("{node} lists the tag `{tag}` twice")]
    RepeatedTag { node: NodeId, tag: TagName },
    /// A former label in a decision that is neither retired nor superseded (RQ-179).
    #[error("decision {decision} is {state} and names the former label {label}")]
    FormerLabelInLiveDecision {
        decision: DecisionId,
        state: &'static str,
        label: Label,
    },
    /// A former label that an element still carries (RQ-180).
    #[error("the former label {label} is a label of {element}")]
    FormerLabelInUse { label: Label, element: ElementId },
    /// A resolved decision with fewer than two options (AC-734).
    #[error("decision {0} is resolved with fewer than two options")]
    TooFewOptions(DecisionId),
    /// A resolved decision without a driver (AC-735).
    #[error("decision {0} is resolved without a driver")]
    NoDriver(DecisionId),
    /// A resolved decision without an assessment of one option against one driver (AC-736).
    #[error("decision {decision} has no assessment of option {option} against driver {driver}")]
    MissingAssessment {
        decision: DecisionId,
        option: OptionId,
        driver: DriverId,
    },
    /// An accepted, superseded or retired decision without a consequence (AC-739).
    #[error("decision {0} has no consequence")]
    NoConsequence(DecisionId),
    /// An accepted, superseded or retired decision without an `outcome` subject (AC-740).
    #[error("decision {0} has no `subject` edge of role `outcome`")]
    NoOutcomeSubject(DecisionId),
}

impl ValidationError {
    /// The keys of the elements the error names, in the order it names them; the places to
    /// report it at.
    #[must_use]
    pub fn keys(&self) -> Vec<Key> {
        match self {
            Self::Integrity(error) => integrity_keys(*error),
            Self::Cycle(cycle) => cycle.0.iter().map(|decision| decision.key()).collect(),
            Self::Binding(error) => binding_keys(error),
            Self::RepeatedTag { node, .. } => vec![node.key()],
            Self::FormerLabelInUse { element, .. } => vec![element.key()],
            Self::FormerLabelInLiveDecision { decision, .. }
            | Self::TooFewOptions(decision)
            | Self::NoDriver(decision)
            | Self::MissingAssessment { decision, .. }
            | Self::NoConsequence(decision)
            | Self::NoOutcomeSubject(decision) => vec![decision.key()],
        }
    }
}

fn integrity_keys(error: IntegrityError) -> Vec<Key> {
    match error {
        IntegrityError::DuplicateKey(key) => vec![key],
        IntegrityError::UnknownEdgeEndpoint(element)
        | IntegrityError::UnknownBindingElement(element) => vec![element.key()],
        IntegrityError::UnknownDecisionReference { decision, .. }
        | IntegrityError::UnknownSuccessor { decision, .. }
        | IntegrityError::SupersedesItself(decision) => vec![decision.key()],
    }
}

fn binding_keys(error: &BindingError) -> Vec<Key> {
    match *error {
        BindingError::Unbound(element)
        | BindingError::AlreadyBound(element)
        | BindingError::PrefixMismatch { element, .. } => vec![element.key()],
        BindingError::LabelUnderTwoKeys { first, second, .. }
        | BindingError::AliasUnderTwoKeys { first, second, .. } => {
            vec![first.key(), second.key()]
        }
        BindingError::AliasIsLiveLabel {
            alias_element,
            label_element,
            ..
        } => vec![alias_element.key(), label_element.key()],
    }
}

/// Every rule the document breaks, in a stable order: integrity, cycles, bindings, nodes and
/// edges, then decisions.
#[must_use]
pub fn validate(document: &Document) -> Vec<ValidationError> {
    let (_, integrity, cycles) = Index::build(document);
    let mut errors: Vec<ValidationError> = integrity.into_iter().map(Into::into).collect();
    errors.extend(cycles.into_iter().map(ValidationError::from));
    errors.extend(
        document
            .bindings
            .validate()
            .into_iter()
            .map(ValidationError::from),
    );
    nodes::check(document, &mut errors);
    decisions::check(document, &mut errors);
    errors
}

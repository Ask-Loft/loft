//! The decisions one document added, changed and removed against its base, and the subjects each
//! gained and lost (the `loft status` and See a spec change Features). A decision's subjects are
//! `subject` edges of its document, not fields of the decision, so they are compared beside it. A
//! node that `loft fmt` names by its former label once it leaves the document is the same node
//! as before: each former label a binding, live or departed, resolves is read as its node before
//! the two versions are compared, in a decision's subjects, drivers and consequences alike.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::bindings::Bindings;
use crate::decision::{Decision, DriverPayload, NodeRef};
use crate::document::{Document, Edge, SubjectRole};
use crate::ids::{DecisionId, ElementId};

/// A node a decision names as a subject, and the role it gives it.
pub type Subject = (NodeRef, SubjectRole);

/// The subjects one decision gained and lost.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubjectChange {
    pub gained: BTreeSet<Subject>,
    pub lost: BTreeSet<Subject>,
}

/// The decisions a document added, changed and removed against its base: the added and changed
/// in the document's order, the removed in the base's; and the subjects each of them gained and
/// lost, where it gained or lost one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecisionChanges {
    pub added: Vec<DecisionId>,
    pub changed: Vec<DecisionId>,
    pub removed: Vec<DecisionId>,
    pub subjects: BTreeMap<DecisionId, SubjectChange>,
}

impl DecisionChanges {
    /// Whether the document holds every decision of its base unchanged and no other.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }
}

/// The decisions `document` added, changed and removed against `base`. A decision changed where
/// its record or its subjects differ, once each former label is read as the node it names.
#[must_use]
pub fn decision_changes(base: &Document, document: &Document) -> DecisionChanges {
    let (then, now) = (Read::of(base), Read::of(document));
    let mut changes = DecisionChanges::default();
    for decision in &document.decisions {
        let id = decision.id;
        let list = match then.decisions.get(&id) {
            None => &mut changes.added,
            Some(was) if *was != now.decisions[&id] || then.subjects(id) != now.subjects(id) => {
                &mut changes.changed
            }
            Some(_) => continue,
        };
        list.push(id);
    }
    changes.removed = (base.decisions.iter())
        .map(|decision| decision.id)
        .filter(|id| !now.decisions.contains_key(id))
        .collect();
    let decided = changes.added.iter().chain(&changes.changed);
    for id in decided.chain(&changes.removed) {
        let (was, is) = (then.subjects(*id), now.subjects(*id));
        let change = SubjectChange {
            gained: is.difference(was).copied().collect(),
            lost: was.difference(is).copied().collect(),
        };
        if !change.gained.is_empty() || !change.lost.is_empty() {
            changes.subjects.insert(*id, change);
        }
    }
    changes
}

/// A document's decisions, each former label read as its node, by ID, and each one's subjects.
struct Read<'a> {
    decisions: BTreeMap<DecisionId, Cow<'a, Decision>>,
    subjects: BTreeMap<DecisionId, BTreeSet<Subject>>,
    none: BTreeSet<Subject>,
}

impl<'a> Read<'a> {
    fn of(document: &'a Document) -> Self {
        let bindings = &document.bindings;
        let mut subjects: BTreeMap<DecisionId, BTreeSet<Subject>> = BTreeMap::new();
        for edge in &document.edges {
            if let Edge::Subject {
                decision,
                subject,
                role,
            } = *edge
            {
                let subject = (resolved(subject, bindings), role);
                subjects.entry(decision).or_default().insert(subject);
            }
        }
        Self {
            decisions: (document.decisions.iter())
                .map(|decision| (decision.id, normalized(decision, bindings)))
                .collect(),
            subjects,
            none: BTreeSet::new(),
        }
    }

    /// The subjects of the decision `id`, none where it names none or the document lacks it.
    fn subjects(&self, id: DecisionId) -> &BTreeSet<Subject> {
        self.subjects.get(&id).unwrap_or(&self.none)
    }
}

/// `decision` with each former label of its drivers and consequences read as its node, cloned
/// only where one is.
fn normalized<'a>(decision: &'a Decision, bindings: &Bindings) -> Cow<'a, Decision> {
    let drivers = decision
        .drivers
        .iter()
        .filter_map(|driver| match driver.payload {
            DriverPayload::Subject { subject } => Some(subject),
            DriverPayload::Local { .. } => None,
        });
    let consequences = decision.consequences.iter().filter_map(|each| each.subject);
    let mut references = drivers.chain(consequences);
    if references.all(|reference| resolved(reference, bindings) == reference) {
        return Cow::Borrowed(decision);
    }
    let mut decision = decision.clone();
    for driver in &mut decision.drivers {
        if let DriverPayload::Subject { subject } = &mut driver.payload {
            *subject = resolved(*subject, bindings);
        }
    }
    for consequence in &mut decision.consequences {
        consequence.subject = consequence
            .subject
            .map(|subject| resolved(subject, bindings));
    }
    Cow::Owned(decision)
}

/// The node `reference` names: a former label that a binding, live or departed, gives to a node
/// is read as that node, and any other reference as it is.
fn resolved(reference: NodeRef, bindings: &Bindings) -> NodeRef {
    let NodeRef::Former(label) = reference else {
        return reference;
    };
    match bindings.resolve(label).or_else(|| bindings.former(label)) {
        Some(ElementId::Node(node)) => NodeRef::Held(node),
        _ => reference,
    }
}

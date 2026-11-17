//! The subjects each decision gained and lost between the base and the tree, for a `spec-change`
//! view (the See a spec change Feature). A decision's subjects are `subject` edges of its
//! document, not fields of the decision, so comparing the decisions alone misses them.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::decision::NodeRef;
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::identity::{Key, Label};

/// The node a subject edge names: by its key, or by a former label that no binding resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Subject {
    Node(Key),
    Former(Label),
}

/// A subject and the role the decision gives it.
pub type Named = (Subject, SubjectRole);

/// What one decision's subjects gained and lost.
#[derive(Debug, Default)]
pub struct Change {
    pub gained: BTreeSet<Named>,
    pub lost: BTreeSet<Named>,
}

/// Each decision whose subjects differ between `base` and `document`, by its key. A decision
/// added gains every subject it names, and a decision removed loses every one.
pub fn changes(base: &Document, document: &Document) -> BTreeMap<Key, Change> {
    let (before, after) = (of(base), of(document));
    let none = BTreeSet::new();
    let decisions: BTreeSet<Key> = before.keys().chain(after.keys()).copied().collect();
    decisions
        .into_iter()
        .filter_map(|key| {
            let was = before.get(&key).unwrap_or(&none);
            let is = after.get(&key).unwrap_or(&none);
            let change = Change {
                gained: is.difference(was).copied().collect(),
                lost: was.difference(is).copied().collect(),
            };
            (!change.gained.is_empty() || !change.lost.is_empty()).then_some((key, change))
        })
        .collect()
}

/// The subjects of each decision of `document`, by the decision's key.
fn of(document: &Document) -> BTreeMap<Key, BTreeSet<Named>> {
    let mut subjects: BTreeMap<Key, BTreeSet<Named>> = BTreeMap::new();
    for edge in &document.edges {
        if let Edge::Subject {
            decision,
            subject,
            role,
        } = *edge
        {
            let named = (subject_of(document, subject), role);
            subjects.entry(decision.key()).or_default().insert(named);
        }
    }
    subjects
}

/// The node `subject` names. A former label is the node its binding, live or departed, keeps,
/// so that a node `loft fmt` names by its former label once it is removed is the same subject.
fn subject_of(document: &Document, subject: NodeRef) -> Subject {
    match subject {
        NodeRef::Held(node) => Subject::Node(node.key()),
        NodeRef::Former(label) => document
            .bindings
            .resolve(label)
            .or_else(|| {
                document
                    .bindings
                    .departed()
                    .find(|binding| binding.label == label)
                    .map(|binding| binding.element)
            })
            .map_or(Subject::Former(label), |element| {
                Subject::Node(element.key())
            }),
    }
}

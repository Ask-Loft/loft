//! The subjects each decision gained and lost between the base and the tree, for a `spec-change`
//! view (the See a spec change Feature). A decision's subjects are `subject` edges of its
//! document, not fields of the decision, so comparing the decisions alone misses them.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::decision::NodeRef;
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::identity::{Key, Label};
use loftspec::ids::ElementId;

/// The node a subject edge names: by its key, or by a former label that no binding resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Subject {
    Node(Key),
    Former(Label),
}

/// A subject and the role the decision gives it.
pub type Named = (Subject, SubjectRole);

/// What one decision's subjects gained and lost.
#[derive(Debug)]
pub struct Change {
    pub gained: BTreeSet<Named>,
    pub lost: BTreeSet<Named>,
    /// The former label of each subject a former label names, by its node's key: the node is in
    /// neither document, so that label is all there is to show of it.
    pub formers: BTreeMap<Key, Label>,
}

/// The subjects of each decision of a document, by the decision's key, and the former label of
/// each node a subject names by one, by the node's key.
struct Subjects {
    named: BTreeMap<Key, BTreeSet<Named>>,
    formers: BTreeMap<Key, Label>,
}

/// Each decision whose subjects differ between `base` and `document`, by its key. A decision
/// added gains every subject it names, and a decision removed loses every one.
pub fn changes(base: &Document, document: &Document) -> BTreeMap<Key, Change> {
    let (before, after) = (Subjects::of(base), Subjects::of(document));
    let none = BTreeSet::new();
    let decisions: BTreeSet<Key> = before
        .named
        .keys()
        .chain(after.named.keys())
        .copied()
        .collect();
    decisions
        .into_iter()
        .filter_map(|key| {
            let was = before.named.get(&key).unwrap_or(&none);
            let is = after.named.get(&key).unwrap_or(&none);
            let gained: BTreeSet<Named> = is.difference(was).copied().collect();
            let lost: BTreeSet<Named> = was.difference(is).copied().collect();
            if gained.is_empty() && lost.is_empty() {
                return None;
            }
            let formers = gained
                .iter()
                .chain(&lost)
                .filter_map(|(subject, _)| match subject {
                    Subject::Node(held) => {
                        let label = after.formers.get(held).or(before.formers.get(held));
                        label.map(|label| (*held, *label))
                    }
                    Subject::Former(_) => None,
                })
                .collect();
            Some((
                key,
                Change {
                    gained,
                    lost,
                    formers,
                },
            ))
        })
        .collect()
}

/// The key of each node a decision of `changes` gained as a subject, where a key names it.
pub fn gained_nodes(changes: &BTreeMap<Key, Change>) -> impl Iterator<Item = Key> + '_ {
    changes
        .values()
        .flat_map(|change| &change.gained)
        .filter_map(|(subject, _)| match subject {
            Subject::Node(key) => Some(*key),
            Subject::Former(_) => None,
        })
}

impl Subjects {
    fn of(document: &Document) -> Self {
        let bindings = &document.bindings;
        // A departed label is never given again, so each names one node.
        let departed: BTreeMap<Label, Key> = bindings
            .departed()
            .map(|binding| (binding.label, binding.element.key()))
            .collect();
        let mut subjects = Self {
            named: BTreeMap::new(),
            formers: BTreeMap::new(),
        };
        for edge in &document.edges {
            if let Edge::Subject {
                decision,
                subject,
                role,
            } = *edge
            {
                let subject = match subject {
                    NodeRef::Held(node) => Subject::Node(node.key()),
                    NodeRef::Former(label) => subjects.former(document, &departed, label),
                };
                subjects
                    .named
                    .entry(decision.key())
                    .or_default()
                    .insert((subject, role));
            }
        }
        subjects
    }

    /// The node a former label names: the node its binding, live or departed, keeps, so that a
    /// node `loft fmt` names by its former label once it is removed is the same subject.
    fn former(
        &mut self,
        document: &Document,
        departed: &BTreeMap<Label, Key>,
        label: Label,
    ) -> Subject {
        let key = document
            .bindings
            .resolve(label)
            .map(ElementId::key)
            .or_else(|| departed.get(&label).copied());
        key.map_or(Subject::Former(label), |key| {
            self.formers.insert(key, label);
            Subject::Node(key)
        })
    }
}

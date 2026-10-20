//! Label allocation at publish (F-28, RQ-7, RQ-8, RQ-63): every node of a kind that takes a
//! label prefix, and every decision, gets the next label of its prefix, in document order, from
//! counters that never rewind and never give out a label again. A key that came back takes back
//! the label it departed with, and the binding of a key that left is marked departed.

use std::collections::BTreeSet;

use crate::bindings::Binding;
use crate::decision::StateKind;
use crate::document::Document;
use crate::hash::decision_history_hashes;
use crate::identity::{Key, Label, LabelPrefix};
use crate::ids::ElementId;
use crate::node::Node;
use crate::validate::decisions::all_former_labels;

/// A label a publish gave an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocated {
    pub label: Label,
    pub element: ElementId,
}

/// What a publish did to the labels: each label it gave back to a key that came back, each label
/// it allocated, and each binding it marked departed, in the order it did them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allocation {
    pub given_back: Vec<Allocated>,
    pub allocated: Vec<Allocated>,
    pub departed: Vec<Allocated>,
}

/// Settles every binding a publish settles: gives each key that came back its departed label,
/// allocates as [`allocate`] does, then marks departed the binding of each key the document no
/// longer holds. No binding is removed.
pub fn publish_labels(document: &mut Document) -> Allocation {
    let elements = elements_in_order(document);
    let mut given_back = Vec::new();
    for element in &elements {
        let key = element.key();
        if !document.bindings.is_departed(key) {
            continue;
        }
        document
            .bindings
            .restore(*element)
            .expect("a departed binding is bound");
        let label = document.bindings.label_of_key(key);
        if let Some(label) = label.filter(|label| Some(label.prefix()) == element.label_prefix()) {
            given_back.push(Allocated {
                label,
                element: *element,
            });
        }
    }
    let allocated = allocate_in(document, &elements);
    let held: BTreeSet<Key> = elements.iter().map(|element| element.key()).collect();
    let leaving: Vec<Allocated> = document
        .bindings
        .live()
        .filter(|binding| !held.contains(&binding.element.key()))
        .map(|binding| Allocated {
            label: binding.label,
            element: binding.element,
        })
        .collect();
    for gone in &leaving {
        document
            .bindings
            .depart(gone.element)
            .expect("a live binding is bound");
    }
    Allocation {
        given_back,
        allocated,
        departed: leaving,
    }
}

/// Gives each element without a label of its kind's prefix the next label of that prefix, and
/// reports what it gave, in the order given. An element whose binding is departed is given that
/// binding back first, as [`publish_labels`] gives it.
///
/// The counters are first raised above every live label, alias and former label, so a counter
/// never rewinds and no label is given out twice (AC-25, AC-26, AC-722). Nodes come in document
/// order: the Products in the manifest's order, then the root Components by file name (AC-24,
/// AC-262), each parent before its children; the decisions follow by file name. An element whose
/// label has another kind's prefix is relabelled, its previous label kept as an alias (RQ-63).
pub fn allocate(document: &mut Document) -> Vec<Allocated> {
    let elements = elements_in_order(document);
    for element in &elements {
        if document.bindings.is_departed(element.key()) {
            document
                .bindings
                .restore(*element)
                .expect("a departed binding is bound");
        }
    }
    allocate_in(document, &elements)
}

/// Allocates over `elements`, in their order, none of whose bindings is departed.
fn allocate_in(document: &mut Document, elements: &[ElementId]) -> Vec<Allocated> {
    raise_counters(document);
    let mut allocated = Vec::new();
    for &element in elements {
        let Some(prefix) = element.label_prefix() else {
            continue;
        };
        let current = document.bindings.label_of(element);
        if current.is_some_and(|label| label.prefix() == prefix) {
            continue;
        }
        let label = next_label(document, prefix);
        let bound = match current {
            Some(_) => document.bindings.relabel(element, label),
            None => document.bindings.bind(element, label),
        };
        bound.expect("a fresh label of the element's own prefix binds");
        allocated.push(Allocated { label, element });
    }
    allocated
}

/// Records in each decision's binding the kind of its state, with its decision history hash for
/// a decision that is not proposed, and removes both from the binding of each departed decision.
///
/// # Panics
///
/// The document does not index: a publish records only a document the validator accepts.
pub fn record_decisions(document: &mut Document) {
    let histories = decision_history_hashes(&document.index().expect("a valid document indexes"));
    for decision in &document.decisions {
        let kind = decision.state.kind();
        let history = (kind != StateKind::Proposed).then(|| histories[&decision.id]);
        let recorded = document
            .bindings
            .record_published(decision.id, Some(kind), history);
        recorded.expect("every decision has a binding after allocation");
    }
    let departed: Vec<ElementId> = document
        .bindings
        .iter()
        .filter(|binding| binding.departed && matches!(binding.element, ElementId::Decision(_)))
        .map(|binding| binding.element)
        .collect();
    for element in departed {
        let cleared = document.bindings.record_published(element, None, None);
        cleared.expect("a departed binding is bound");
    }
}

fn raise_counters(document: &mut Document) {
    let Document {
        bindings,
        counters,
        edges,
        decisions,
        ..
    } = document;
    let live = bindings.iter().flat_map(Binding::every_label);
    counters.raise_all(live.chain(all_former_labels(edges, decisions)));
}

fn next_label(document: &mut Document, prefix: LabelPrefix) -> Label {
    let label = Label::new(prefix, document.counters.next(prefix));
    document.counters.raise(label);
    label
}

/// Every node, then every decision, in the order labels are allocated.
pub(super) fn elements_in_order(document: &Document) -> Vec<ElementId> {
    let mut elements = Vec::new();
    let mut visit = |node: Node<'_>| {
        if let Some(id) = node.id() {
            elements.push(ElementId::Node(id));
        }
    };
    for product in &document.products {
        Node::Product(product).walk(&mut visit);
    }
    let mut components: Vec<_> = document.components.iter().collect();
    components.sort_by(|a, b| a.slug.as_str().cmp(b.slug.as_str()));
    for root in components {
        Node::Component(&root.component).walk(&mut visit);
    }
    let mut decisions: Vec<_> = document.decisions.iter().collect();
    decisions.sort_by(|a, b| a.slug.as_str().cmp(b.slug.as_str()));
    elements.extend(
        decisions
            .into_iter()
            .map(|decision| ElementId::Decision(decision.id)),
    );
    elements
}

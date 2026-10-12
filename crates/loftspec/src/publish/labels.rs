//! Label allocation at publish (F-28, RQ-7, RQ-8, RQ-63): every node of a kind that takes a
//! label prefix, and every decision, gets the next label of its prefix, in document order, from
//! counters that never rewind and never give out a label again.

use crate::bindings::Binding;
use crate::document::Document;
use crate::identity::{Label, LabelPrefix};
use crate::ids::ElementId;
use crate::node::Node;
use crate::validate::decisions::all_former_labels;

/// A label a publish gave an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocated {
    pub label: Label,
    pub element: ElementId,
}

/// Gives each element without a label of its kind's prefix the next label of that prefix, and
/// reports what it gave, in the order given.
///
/// The counters are first raised above every live label, alias and former label, so a counter
/// never rewinds and no label is given out twice (AC-25, AC-26, AC-722). Nodes come in document
/// order: the Products in the manifest's order, then the root Components by file name (AC-24,
/// AC-262), each parent before its children; the decisions follow by file name. An element whose
/// label has another kind's prefix is relabelled, its previous label kept as an alias (RQ-63).
pub fn allocate(document: &mut Document) -> Vec<Allocated> {
    raise_counters(document);
    let mut allocated = Vec::new();
    for element in elements_in_order(document) {
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
fn elements_in_order(document: &Document) -> Vec<ElementId> {
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

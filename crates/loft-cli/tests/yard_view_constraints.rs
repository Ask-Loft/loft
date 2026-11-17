//! What a `spec-change` view records of a changed Constraint or Quality Attribute: a box of its
//! own beneath the box of the node that holds it, which is drawn only for context, and the
//! subject edge of a decision that names it, which ends at its own box.

use serde_json::Value;

use crate::yard::Yard;
use crate::yard_view_reports::{node, parent, report};

/// The Reading List's Product, whose Shelf Feature Group holds a Constraint.
const PRODUCT: &str = "products/reading-list/product.yaml";

/// The Feature that holds a Quality Attribute.
const ADD_A_BOOK: &str = "products/reading-list/shelf/add-a-book.yaml";

/// The Component that holds a Constraint and a Quality Attribute.
const SHELF_STORE: &str = "components/shelf-store.yaml";

/// The Shelf store's Constraint.
const IN_RUST: &str = "The shelf store shall be written in Rust.";

/// The Shelf store's Constraint, reworded.
const IN_SAFE_RUST: &str = "The shelf store shall be written in safe Rust.";

/// The diagram of a `spec-change` view of the tree as it now stands.
fn diagram(yard: &Yard) -> Value {
    report(yard, "spec-change")["graph"].clone()
}

/// The number of boxes `graph` draws.
fn boxes(graph: &Value) -> usize {
    graph["nodes"].as_array().unwrap().len()
}

#[test]
fn a_spec_change_view_draws_a_feature_group_s_changed_constraint_as_its_own_box_beneath_the_group()
{
    let yard = Yard::with_spec();
    yard.scratch.edit(
        PRODUCT,
        "Every book shall carry a title.",
        "Every book shall carry a title and an author.",
    );
    let graph = diagram(&yard);
    let constraint = node(&graph, "Every book shall carry a title and an author.");
    assert_eq!(constraint["kind"], "constraint");
    assert_eq!(constraint["change"], "changed");
    let group = parent(&graph, constraint);
    assert_eq!(group["name"], "Shelf");
    assert_eq!(
        group["change"],
        Value::Null,
        "its own content did not change"
    );
    let product = parent(&graph, group);
    assert_eq!(product["kind"], "product");
    assert_eq!(product["change"], Value::Null, "drawn only for context");
    assert_eq!(boxes(&graph), 3, "{graph}");
}

#[test]
fn a_spec_change_view_draws_a_feature_s_changed_quality_attribute_as_its_own_box_beneath_the_feature()
 {
    let yard = Yard::with_spec();
    yard.scratch.edit(
        ADD_A_BOOK,
        "Adding a book completes quickly.",
        "Adding a book completes within a blink.",
    );
    let graph = diagram(&yard);
    let attribute = node(&graph, "Adding a book completes within a blink.");
    assert_eq!(attribute["kind"], "quality_attribute");
    assert_eq!(attribute["change"], "changed");
    let feature = parent(&graph, attribute);
    assert_eq!(feature["name"], "Add a book");
    assert_eq!(
        feature["change"],
        Value::Null,
        "its own content did not change"
    );
    assert_eq!(
        boxes(&graph),
        4,
        "the attribute and the three boxes above it: {graph}"
    );
}

#[test]
fn a_spec_change_view_draws_a_component_s_changed_constraint_and_quality_attribute_as_boxes_beneath_it()
 {
    let yard = Yard::with_spec();
    yard.scratch.edit(SHELF_STORE, IN_RUST, IN_SAFE_RUST);
    yard.scratch.edit(
        SHELF_STORE,
        "Listing a shelf completes quickly.",
        "Listing a shelf completes within a blink.",
    );
    let graph = diagram(&yard);
    for (name, kind) in [
        (IN_SAFE_RUST, "constraint"),
        (
            "Listing a shelf completes within a blink.",
            "quality_attribute",
        ),
    ] {
        let held = node(&graph, name);
        assert_eq!(held["kind"], kind);
        assert_eq!(held["change"], "changed");
        let component = parent(&graph, held);
        assert_eq!(component["name"], "Shelf store");
        assert_eq!(
            component["change"],
            Value::Null,
            "its own content did not change"
        );
    }
    assert_eq!(boxes(&graph), 3, "{graph}");
}

#[test]
fn a_spec_change_view_draws_an_added_and_a_removed_constraint_as_boxes_of_their_own() {
    let yard = Yard::with_spec();
    yard.scratch.edit(
        PRODUCT,
        "        verificationFate: proved\n",
        "        verificationFate: proved\n      \
         - key: 00000000-0000-4000-8000-0000000000a2\n        \
         name: \"Every book shall carry an author.\"\n",
    );
    yard.scratch.edit(
        SHELF_STORE,
        "constraints:\n  - key: 00000000-0000-4000-8000-00000000003e\n    \
         name: \"The shelf store shall be written in Rust.\"\n    constraintBinding:\n      \
         category: runtime.language\n      requiredValue: \"rust\"\n    \
         verificationFate: monitored\n",
        "",
    );
    // A removed node's label is marked departed, as a removal is written.
    let formatted = yard.scratch.loft(&["fmt"]);
    assert!(formatted.status.success(), "{formatted:?}");
    let graph = diagram(&yard);
    let added = node(&graph, "Every book shall carry an author.");
    assert_eq!(added["kind"], "constraint");
    assert_eq!(added["change"], "added");
    assert_eq!(parent(&graph, added)["name"], "Shelf");
    let removed = node(&graph, IN_RUST);
    assert_eq!(removed["kind"], "constraint");
    assert_eq!(removed["change"], "removed");
    assert_eq!(parent(&graph, removed)["name"], "Shelf store");
}

#[test]
fn a_spec_change_view_ends_a_decision_s_subject_edge_at_the_box_of_the_constraint_it_names() {
    let yard = Yard::with_spec();
    let decision = "decisions/keep-a-change-log.yaml";
    yard.scratch.edit(
        decision,
        "outcome: \"An append-only log.\"",
        "outcome: \"An append-only log, kept a year.\"",
    );
    yard.scratch.edit(
        decision,
        "    node: 00000000-0000-4000-8000-00000000002b\n",
        "    node: 00000000-0000-4000-8000-00000000002b\n  - role: affected\n    \
         node: 00000000-0000-4000-8000-00000000003e\n",
    );
    yard.scratch.edit(SHELF_STORE, IN_RUST, IN_SAFE_RUST);
    let graph = diagram(&yard);
    let constraint = node(&graph, IN_SAFE_RUST);
    let affected: Vec<&Value> = graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| edge["role"] == "affected")
        .collect();
    assert_eq!(affected.len(), 1, "{graph}");
    assert_eq!(affected[0]["kind"], "subject");
    assert_eq!(
        affected[0]["to"], constraint["key"],
        "the Constraint's own box, not its Component's: {graph}"
    );
}

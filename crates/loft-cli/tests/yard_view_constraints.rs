//! What a `spec-change` view records of a changed Constraint or Quality Attribute: a box of its
//! own beneath the box of the node that holds it, which is drawn only for context, and the
//! subject edge of a decision that names it, which ends at its own box.

use std::fs;

use serde_json::Value;

use crate::yard::Yard;
use crate::yard_view_reports::{CHANGE_LOG, change_a_decision, node, parent, report};

/// The Reading List's Product, whose Shelf Feature Group holds a Constraint.
const PRODUCT: &str = "products/reading-list/product.yaml";

/// The Feature that holds a Quality Attribute.
const ADD_A_BOOK: &str = "products/reading-list/shelf/add-a-book.yaml";

/// The Component that holds a Constraint and a Quality Attribute.
const SHELF_STORE: &str = "components/shelf-store.yaml";

/// The Shelf store's Constraint, the only one it holds.
const IN_RUST: &str = "The shelf store shall be written in Rust.";

/// The key of the Shelf store's Constraint.
const IN_RUST_KEY: &str = "00000000-0000-4000-8000-00000000003e";

/// The Shelf store's Constraint, reworded.
const IN_SAFE_RUST: &str = "The shelf store shall be written in safe Rust.";

/// The diagram of a `spec-change` view of the tree as it now stands.
fn diagram(yard: &Yard) -> Value {
    report(yard, "spec-change")["graph"].clone()
}

/// Asserts that `graph` draws the node named `name` as a box of its own, of `kind` and marked
/// `change`, beneath the box of the node named `holder`, which is drawn only for context; gives
/// the holder's box.
fn assert_boxed_beneath<'a>(
    graph: &'a Value,
    name: &str,
    kind: &str,
    change: &str,
    holder: &str,
) -> &'a Value {
    let held = node(graph, name);
    assert_eq!(held["kind"], kind, "{held}");
    assert_eq!(held["change"], change, "{held}");
    let above = parent(graph, held);
    assert_eq!(above["name"], holder, "{graph}");
    assert_eq!(
        above["change"],
        Value::Null,
        "its own content did not change: {above}"
    );
    above
}

#[test]
fn a_spec_change_view_draws_the_changed_constraint_and_quality_attribute_of_a_feature_group_and_a_feature_as_boxes_beneath_them()
 {
    let yard = Yard::with_spec();
    yard.scratch.edit(
        PRODUCT,
        "Every book shall carry a title.",
        "Every book shall carry a title and an author.",
    );
    yard.scratch.edit(
        ADD_A_BOOK,
        "Adding a book completes quickly.",
        "Adding a book completes within a blink.",
    );
    let graph = diagram(&yard);
    let group = assert_boxed_beneath(
        &graph,
        "Every book shall carry a title and an author.",
        "constraint",
        "changed",
        "Shelf",
    );
    assert_boxed_beneath(
        &graph,
        "Adding a book completes within a blink.",
        "quality_attribute",
        "changed",
        "Add a book",
    );
    let product = parent(&graph, group);
    assert_eq!(product["kind"], "product");
    assert_eq!(product["change"], Value::Null, "drawn only for context");
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 5, "{graph}");
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
    assert_boxed_beneath(&graph, IN_SAFE_RUST, "constraint", "changed", "Shelf store");
    assert_boxed_beneath(
        &graph,
        "Listing a shelf completes within a blink.",
        "quality_attribute",
        "changed",
        "Shelf store",
    );
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 3, "{graph}");
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
    let path = yard.scratch.file(SHELF_STORE);
    let store = fs::read_to_string(&path).unwrap();
    let (start, end) = (
        store.find("constraints:\n").unwrap(),
        store.find("qualityAttributes:\n").unwrap(),
    );
    let cut = &store[start..end];
    assert!(
        cut.contains(IN_RUST_KEY) && cut.matches("- key:").count() == 1,
        "the cut holds the Shelf store's one Constraint: {cut}"
    );
    fs::write(&path, format!("{}{}", &store[..start], &store[end..])).unwrap();
    // A removed node's label is marked departed, as a removal is written.
    let formatted = yard.scratch.loft(&["fmt"]);
    assert!(formatted.status.success(), "{formatted:?}");
    let graph = diagram(&yard);
    assert_boxed_beneath(
        &graph,
        "Every book shall carry an author.",
        "constraint",
        "added",
        "Shelf",
    );
    assert_boxed_beneath(&graph, IN_RUST, "constraint", "removed", "Shelf store");
}

#[test]
fn a_spec_change_view_ends_a_decision_s_subject_edge_at_the_box_of_the_constraint_it_names() {
    let yard = Yard::with_spec();
    // A subject is an edge, not a field of its decision: the decision and its edges are drawn
    // only once its own words change.
    change_a_decision(&yard);
    yard.scratch.edit(
        CHANGE_LOG,
        "subjects:\n",
        &format!("subjects:\n  - role: affected\n    node: {IN_RUST_KEY}\n"),
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

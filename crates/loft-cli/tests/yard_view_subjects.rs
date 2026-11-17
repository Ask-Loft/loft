//! What a `spec-change` view records of a decision whose subjects changed: the decision drawn as
//! changed, each subject it gained drawn so that its edge ends at a box, no edge to a subject it
//! lost, and its diff entry listing the subjects added and removed.

use std::fs;

use serde_json::Value;

use crate::yard::Yard;
use crate::yard_view_reports::{CHANGE_LOG, node, parent, report};

/// The subjects of Keep a change log: Requirement RQ-2 as its outcome.
const OUTCOME_ONLY: &str =
    "subjects:\n  - role: outcome\n    node: 00000000-0000-4000-8000-00000000002b\n";

/// The key of criterion AC-3, under RQ-2, which no decision names.
const IN_ORDER_KEY: &str = "00000000-0000-4000-8000-00000000002c";

/// Criterion AC-3's name.
const IN_ORDER: &str = "Books list in the order they were added.";

/// The key of Requirement RQ-2.
const KEEP_ORDER_KEY: &str = "00000000-0000-4000-8000-00000000002b";

/// The diagram of a `spec-change` view of the tree as it now stands.
fn diagram(yard: &Yard) -> Value {
    report(yard, "spec-change")["graph"].clone()
}

/// Names criterion AC-3 as affected by Keep a change log, beside its outcome.
fn name_in_order(yard: &Yard) {
    yard.scratch.edit(
        CHANGE_LOG,
        OUTCOME_ONLY,
        &format!("{OUTCOME_ONLY}  - role: affected\n    node: {IN_ORDER_KEY}\n"),
    );
}

/// The entry of the decision named `title`, in the diagram's decisions and in its diff.
fn decision<'a>(graph: &'a Value, title: &str) -> (&'a Value, &'a Value) {
    let find = |list: &'a Value| {
        list.as_array()
            .unwrap()
            .iter()
            .find(|each| each["kind"] == "decision" && each["name"] == title)
            .unwrap_or_else(|| panic!("no decision {title}: {graph}"))
    };
    (find(&graph["decisions"]), find(&graph["diff"]))
}

/// The subject edges of `graph`, as (from, to, role).
fn subject_edges(graph: &Value) -> Vec<(String, String, String)> {
    graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| edge["kind"] == "subject")
        .map(|edge| {
            let text = |field: &str| edge[field].as_str().unwrap().to_owned();
            (text("from"), text("to"), text("role"))
        })
        .collect()
}

#[test]
fn a_spec_change_view_draws_a_decision_whose_only_change_is_a_subject_added_as_changed() {
    let yard = Yard::with_spec();
    name_in_order(&yard);
    let graph = diagram(&yard);
    let (drawn, entry) = decision(&graph, "Keep a change log");
    assert_eq!(drawn["change"], "changed", "{graph}");
    assert_eq!(entry["change"], "changed", "{graph}");
    assert_eq!(entry["words"]["before"], entry["words"]["after"]);
    assert_eq!(
        entry["subjects"],
        serde_json::json!([{
            "key": IN_ORDER_KEY,
            "kind": "criterion",
            "label": "AC-3",
            "name": IN_ORDER,
            "role": "affected",
            "change": "added",
        }]),
        "{entry}"
    );
}

#[test]
fn a_spec_change_view_draws_the_subject_a_decision_gained_only_for_context_with_the_nodes_above_it()
{
    let yard = Yard::with_spec();
    name_in_order(&yard);
    let graph = diagram(&yard);
    let criterion = node(&graph, IN_ORDER);
    assert_eq!(criterion["change"], Value::Null, "{graph}");
    let mut above = parent(&graph, criterion);
    for kind in ["requirement", "feature", "feature_group", "product"] {
        assert_eq!(above["kind"], kind, "{graph}");
        assert_eq!(
            above["change"],
            Value::Null,
            "drawn only for context: {above}"
        );
        if kind != "product" {
            above = parent(&graph, above);
        }
    }
    let (drawn, _) = decision(&graph, "Keep a change log");
    let from = drawn["key"].as_str().unwrap().to_owned();
    assert!(
        subject_edges(&graph).contains(&(from, IN_ORDER_KEY.to_owned(), "affected".to_owned())),
        "the added subject edge ends at the criterion's box: {graph}"
    );
}

#[test]
fn a_spec_change_view_draws_no_edge_to_a_subject_a_decision_lost_and_lists_it_as_removed() {
    let yard = Yard::with_spec();
    yard.scratch.edit(CHANGE_LOG, OUTCOME_ONLY, "");
    // RQ-2 changes too, so that its box is drawn and an edge to it could be.
    yard.scratch.edit(
        "products/reading-list/shelf/add-a-book.yaml",
        "keep books in the order they were added.",
        "keep books in the order a reader added them.",
    );
    let graph = diagram(&yard);
    assert_eq!(
        node(
            &graph,
            "The shelf shall keep books in the order a reader added them."
        )["key"],
        KEEP_ORDER_KEY
    );
    let (drawn, entry) = decision(&graph, "Keep a change log");
    assert_eq!(drawn["change"], "changed", "{graph}");
    assert_eq!(subject_edges(&graph), Vec::new(), "{graph}");
    let subjects = entry["subjects"].as_array().unwrap();
    assert_eq!(subjects.len(), 1, "{entry}");
    assert_eq!(subjects[0]["label"], "RQ-2");
    assert_eq!(subjects[0]["role"], "outcome");
    assert_eq!(subjects[0]["change"], "removed");
}

#[test]
fn a_spec_change_view_lists_no_subject_where_a_removed_node_is_named_by_its_former_label() {
    let mut yard = Yard::with_spec();
    name_in_order(&yard);
    yard.rebased();
    let path = yard
        .scratch
        .file("products/reading-list/shelf/add-a-book.yaml");
    let feature = fs::read_to_string(&path).unwrap();
    let criterion = format!("      - key: {IN_ORDER_KEY}\n        name: \"{IN_ORDER}\"\n");
    assert!(feature.contains(&criterion), "{feature}");
    fs::write(&path, feature.replace(&criterion, "")).unwrap();
    let formatted = yard.scratch.loft(&["fmt"]);
    assert!(formatted.status.success(), "{formatted:?}");
    let decision_file = fs::read_to_string(yard.scratch.file(CHANGE_LOG)).unwrap();
    assert!(
        decision_file.contains("formerLabel: AC-3"),
        "loft fmt names the removed criterion by its former label: {decision_file}"
    );
    let graph = diagram(&yard);
    assert_eq!(graph["decisions"], serde_json::json!([]), "{graph}");
}

#[test]
fn a_spec_change_view_draws_the_subjects_of_an_added_decision() {
    let yard = Yard::with_spec();
    fs::write(
        yard.scratch.file("decisions/sort-books-by-title.yaml"),
        format!(
            "key: 00000000-0000-4000-8000-0000000000c1\nslug: sort-books-by-title\n\
             title: \"Sort books by title\"\ncontext: \"Readers browse by title.\"\n\
             state:\n  kind: proposed\nsubjects:\n  - role: affected\n    node: {IN_ORDER_KEY}\n"
        ),
    )
    .unwrap();
    let graph = diagram(&yard);
    let (drawn, entry) = decision(&graph, "Sort books by title");
    assert_eq!(drawn["change"], "added");
    assert_eq!(entry["subjects"][0]["change"], "added", "{entry}");
    assert_eq!(node(&graph, IN_ORDER)["change"], Value::Null);
    let from = drawn["key"].as_str().unwrap().to_owned();
    assert_eq!(
        subject_edges(&graph),
        vec![(from, IN_ORDER_KEY.to_owned(), "affected".to_owned())],
        "{graph}"
    );
}

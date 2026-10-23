//! What a `spec-change` and a `publish` view record beside the document the matching command
//! prints, for the page to draw: the boxes of the spec change's diagram and the edges between
//! them, and the publish's comparison with the commit that last wrote the lock and its labelled
//! nodes counted by kind.

use serde_json::Value;

use software_plant::GitScenario;

use crate::common::tree;
use crate::yard::{Yard, block};

/// The commit that last wrote the lock, in the tests that script one.
const PUBLISHED: &str = "5ab1e0005ab1e0005ab1e0005ab1e0005ab1e000";

/// The feature file the tests change.
const SHARE: &str = "products/reading-list/sharing/share-a-list.yaml";

/// The Component that Share a list realizes.
const LINK_SERVICE: &str = "components/link-service.yaml";

/// Records a `loft` block `v1` of `view`, and gives its report.
fn report(yard: &Yard, view: &str) -> Value {
    let batch = format!(
        r#"[{{"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"}},
            {{"id": "v1", "kind": "loft", "stage": "shape", "view": "{view}"}}]"#
    );
    let written = yard.write("s1", &batch, &[]);
    assert_eq!(written["outcome"], "written", "{written}");
    let read = yard.read("s1", &["--reports"]);
    block(&read, "v1")["version"]["body"]["report"].clone()
}

fn change_a_criterion(yard: &Yard) {
    yard.scratch.edit(
        SHARE,
        "An edit made through the link is refused.",
        "An edit made through the link is refused, and the link says so.",
    );
}

/// The box of the node named `name`.
fn node<'a>(graph: &'a Value, name: &str) -> &'a Value {
    graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["name"] == name)
        .unwrap_or_else(|| panic!("no box {name}: {graph}"))
}

/// `scenario` with the tree as it now is published at [`PUBLISHED`], the last commit to write
/// the lock.
fn published_now(yard: &Yard, scenario: GitScenario) -> GitScenario {
    let mut scenario = scenario
        .base(PUBLISHED, PUBLISHED)
        .lock_written_at(PUBLISHED);
    for (number, (path, bytes)) in tree(&yard.scratch.0).iter().enumerate() {
        let text = String::from_utf8(bytes.clone()).unwrap();
        let object = format!("{:040x}", number + 0x1000);
        scenario = scenario.file_at(PUBLISHED, &path.to_string(), &object, &text);
    }
    scenario
}

/// The box whose key is `node`'s parent.
fn parent<'a>(graph: &'a Value, node: &Value) -> &'a Value {
    graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|above| above["key"] == node["parent"])
        .unwrap_or_else(|| panic!("no box above {node}: {graph}"))
}

/// The entry of the diff for the node or decision named `name`.
fn diffed<'a>(graph: &'a Value, name: &str) -> &'a Value {
    graph["diff"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap_or_else(|| panic!("no diff of {name}: {graph}"))
}

/// The statement of the Requirement above the criterion [`change_a_criterion`] changes.
const REFUSE_EDITS: &str =
    "While a list is shared, the shelf shall refuse edits made through the link.";

#[test]
fn a_spec_change_view_draws_a_changed_criterion_under_its_requirement_feature_feature_group_and_product()
 {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let graph = report(&yard, "spec-change")["graph"].clone();
    let criterion = node(
        &graph,
        "An edit made through the link is refused, and the link says so.",
    );
    assert_eq!(criterion["kind"], "criterion");
    assert_eq!(criterion["change"], "changed");
    let requirement = parent(&graph, criterion);
    assert_eq!(requirement["kind"], "requirement");
    assert_eq!(
        requirement["name"], REFUSE_EDITS,
        "a Requirement is named by its statement"
    );
    assert_eq!(requirement["change"], Value::Null, "drawn only for context");
    let feature = parent(&graph, requirement);
    assert_eq!(feature["name"], "Share a list");
    assert_eq!(
        feature["change"],
        Value::Null,
        "its own content did not change"
    );
    let group = parent(&graph, feature);
    assert_eq!(group["kind"], "feature_group");
    assert_eq!(group["change"], Value::Null);
    let product = parent(&graph, group);
    assert_eq!(product["kind"], "product");
    assert_eq!(product["change"], Value::Null);
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 5, "{graph}");
}

#[test]
fn a_spec_change_view_draws_no_unchanged_requirement_and_counts_none_on_a_feature() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let graph = report(&yard, "spec-change")["graph"].clone();
    let names: Vec<&Value> = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| &node["name"])
        .collect();
    assert!(
        !names.contains(&&Value::from(
            "Where sharing is enabled, the shelf shall publish a read-only link."
        )),
        "{names:?}"
    );
    let feature = node(&graph, "Share a list");
    assert!(feature.get("requirements").is_none(), "{feature}");
    assert!(feature.get("criteria").is_none(), "{feature}");
    assert!(feature.get("delta").is_none(), "{feature}");
    assert!(
        feature["label"].is_string(),
        "a published node keeps its label"
    );
}

#[test]
fn a_spec_change_view_s_diff_gives_a_changed_criterion_s_words_before_and_after() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let graph = report(&yard, "spec-change")["graph"].clone();
    let entry = diffed(
        &graph,
        "An edit made through the link is refused, and the link says so.",
    );
    assert_eq!(entry["kind"], "criterion");
    assert_eq!(entry["change"], "changed");
    assert!(entry["label"].is_string(), "{entry}");
    assert_eq!(
        entry["words"]["before"],
        "An edit made through the link is refused."
    );
    assert_eq!(
        entry["words"]["after"],
        "An edit made through the link is refused, and the link says so."
    );
}

#[test]
fn a_spec_change_view_s_diff_gives_an_added_node_s_words_after_and_a_removed_one_s_before() {
    let yard = Yard::with_spec();
    yard.scratch.edit(
        SHARE,
        "        name: \"The link opens the list read-only, as RQ-99 once required.\"\n",
        "        name: \"The link opens the list read-only, as RQ-99 once required.\"\n      \
         - key: 00000000-0000-4000-8000-0000000000a1\n        name: \"The link names the shelf.\"\n",
    );
    yard.scratch.edit(
        SHARE,
        "    criteria:\n      - key: 00000000-0000-4000-8000-000000000036\n        \
         name: \"An edit made through the link is refused.\"\n",
        "",
    );
    // A removed node's label is marked departed, as a removal is written.
    let formatted = yard.scratch.loft(&["fmt"]);
    assert!(formatted.status.success(), "{formatted:?}");
    let graph = report(&yard, "spec-change")["graph"].clone();
    let added = diffed(&graph, "The link names the shelf.");
    assert_eq!(added["change"], "added");
    assert_eq!(
        added["words"],
        serde_json::json!({ "after": "The link names the shelf." })
    );
    let removed = diffed(&graph, "An edit made through the link is refused.");
    assert_eq!(removed["change"], "removed");
    assert_eq!(
        removed["words"],
        serde_json::json!({ "before": "An edit made through the link is refused." })
    );
    assert_eq!(node(&graph, "The link names the shelf.")["change"], "added");
    assert_eq!(
        node(&graph, "An edit made through the link is refused.")["change"],
        "removed"
    );
}

#[test]
fn a_spec_change_view_s_diff_gives_a_decision_s_title_and_outcome() {
    let yard = Yard::with_spec();
    yard.scratch.edit(
        "decisions/keep-a-change-log.yaml",
        "outcome: \"An append-only log.\"",
        "outcome: \"An append-only log, kept a year.\"",
    );
    let graph = report(&yard, "spec-change")["graph"].clone();
    let entry = diffed(&graph, "Keep a change log");
    assert_eq!(entry["kind"], "decision");
    assert_eq!(entry["change"], "changed");
    assert_eq!(
        entry["words"]["before"],
        "Keep a change log\n\nAn append-only log."
    );
    assert_eq!(
        entry["words"]["after"],
        "Keep a change log\n\nAn append-only log, kept a year."
    );
}

#[test]
fn a_spec_change_view_s_diff_gives_the_same_words_before_and_after_where_only_tags_changed() {
    let yard = Yard::with_spec();
    yard.scratch
        .edit(SHARE, "    tags:\n      - portable\n", "");
    let graph = report(&yard, "spec-change")["graph"].clone();
    let entry = diffed(&graph, REFUSE_EDITS);
    assert_eq!(entry["change"], "changed");
    assert_eq!(entry["words"]["before"], REFUSE_EDITS);
    assert_eq!(entry["words"]["after"], REFUSE_EDITS);
}

#[test]
fn a_spec_change_view_draws_the_realizes_edge_between_two_changed_boxes() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    yard.scratch.edit(
        LINK_SERVICE,
        "name: \"Link service\"",
        "name: \"Link services\"",
    );
    let graph = report(&yard, "spec-change")["graph"].clone();
    let feature = node(&graph, "Share a list");
    let component = node(&graph, "Link services");
    assert_eq!(component["change"], "changed");
    let edges = graph["edges"].as_array().unwrap();
    assert!(
        edges.iter().any(|edge| edge["kind"] == "realizes"
            && edge["from"] == feature["key"]
            && edge["to"] == component["key"]),
        "{edges:?}"
    );
}

#[test]
fn a_spec_change_view_of_no_change_draws_no_box() {
    let yard = Yard::with_spec();
    let graph = report(&yard, "spec-change")["graph"].clone();
    assert_eq!(graph["nodes"], serde_json::json!([]));
    assert_eq!(graph["edges"], serde_json::json!([]));
}

#[test]
fn a_publish_view_compares_the_tree_with_the_commit_that_last_wrote_the_lock() {
    let mut yard = Yard::with_spec();
    yard.git = published_now(&yard, yard.git.clone());
    change_a_criterion(&yard);
    let status = report(&yard, "publish")["status"].clone();
    assert_eq!(status, yard.loft(&["status", "--base", PUBLISHED]));
    assert_eq!(status["base"]["changed"].as_array().unwrap().len(), 1);
}

#[test]
fn a_publish_view_where_no_commit_wrote_the_lock_holds_what_loft_status_prints() {
    let yard = Yard::with_spec();
    let status = report(&yard, "publish")["status"].clone();
    assert_eq!(status, yard.loft(&["status"]));
}

#[test]
fn a_publish_view_counts_the_labelled_nodes_and_decisions_by_kind() {
    let yard = Yard::with_spec();
    let labels = report(&yard, "publish")["labels"].clone();
    for kind in [
        "product",
        "feature_group",
        "feature",
        "requirement",
        "criterion",
    ] {
        assert!(labels[kind].as_u64().unwrap() > 0, "{kind}: {labels}");
    }
    assert!(labels["decision"].as_u64().unwrap() > 0, "{labels}");
}

#[test]
fn a_trace_view_gives_the_reason_of_each_criterion_linked_unverified() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let changed = "An edit made through the link is refused, and the link says so.";
    let text = std::fs::read_to_string(yard.scratch.0.join("spec").join(SHARE)).unwrap();
    let key = text[..text.find(changed).unwrap()]
        .rsplit("key: ")
        .next()
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap()
        .to_owned();
    let trace = format!(
        "format: loft-trace/2\nlinks:\n  - node: {key}\n    unverified: \"Read by hand\"\n"
    );
    std::fs::write(yard.scratch.0.join("loft.trace.yaml"), trace).unwrap();
    let report = report(&yard, "trace");
    let criteria: Vec<&Value> = report["features"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|feature| feature["criteria"].as_array().unwrap())
        .collect();
    let linked = criteria
        .iter()
        .find(|criterion| criterion["key"] == key.as_str())
        .unwrap_or_else(|| panic!("no criterion {key}: {report}"));
    assert_eq!(linked["unverified"], "Read by hand");
    assert!(
        criteria
            .iter()
            .filter(|criterion| criterion["key"] != key.as_str())
            .all(|criterion| criterion.get("unverified").is_none()),
        "{report}"
    );
}

#[test]
fn a_findings_view_names_each_node_a_finding_names_by_its_label_and_name() {
    let yard = Yard::with_spec();
    let report = report(&yard, "findings");
    let findings = report["check"]["findings"].as_array().unwrap();
    let subjects: Vec<&str> = findings
        .iter()
        .flat_map(|finding| finding["subjects"].as_array().into_iter().flatten())
        .filter_map(|subject| subject["key"].as_str())
        .collect();
    assert!(
        !subjects.is_empty(),
        "the fixture has findings that name nodes: {report}"
    );
    for key in subjects {
        let named = &report["nodes"][key];
        assert!(named["name"].is_string(), "{key}: {named}");
        assert!(named.get("label").is_some(), "{key}: {named}");
    }
}

#[test]
fn a_trace_view_records_each_feature_s_requirements_by_their_statements_and_each_criterion_s_requirement()
 {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let report = report(&yard, "trace");
    let feature = &report["features"][0];
    let requirements = feature["requirements"].as_array().unwrap();
    let shared = requirements
        .iter()
        .find(|requirement| requirement["key"] == "00000000-0000-4000-8000-000000000035")
        .unwrap_or_else(|| panic!("no requirement: {feature}"));
    assert_eq!(
        shared["name"],
        "While a list is shared, the shelf shall refuse edits made through the link."
    );
    assert_eq!(shared["statement"]["form"], "state_driven");
    assert_eq!(shared["statement"]["condition"], "a list is shared");
    assert_eq!(shared["statement"]["system"], "the shelf");
    assert_eq!(
        shared["statement"]["shall"],
        "refuse edits made through the link."
    );
    let criterion = feature["criteria"]
        .as_array()
        .unwrap()
        .iter()
        .find(|criterion| criterion["key"] == "00000000-0000-4000-8000-000000000036")
        .unwrap();
    assert_eq!(criterion["requirement"], shared["key"]);
}

#[test]
fn a_trace_view_records_each_criterion_with_its_own_tests_and_the_rules_reconcile_reported_of_it() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let trace = "format: loft-trace/2\nlinks:\n  - node: 00000000-0000-4000-8000-000000000036\n    tests:\n      - file: \"tests/share.rs\"\n        name: \"an_edit_through_the_link_is_refused\"\n";
    std::fs::write(yard.scratch.0.join("loft.trace.yaml"), trace).unwrap();
    let report = report(&yard, "trace");
    let reconcile = yard.loft(&["reconcile"]);
    let of = |list: &str, key: &Value, field: &str| -> Vec<Value> {
        reconcile[list]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["key"] == *key)
            .map(|entry| entry[field].clone())
            .collect()
    };
    let criteria: Vec<&Value> = report["features"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|feature| feature["criteria"].as_array().unwrap())
        .collect();
    let linked = criteria
        .iter()
        .find(|criterion| criterion["key"] == "00000000-0000-4000-8000-000000000036")
        .unwrap();
    assert_eq!(
        linked["tests"][0]["name"], "an_edit_through_the_link_is_refused",
        "{linked}"
    );
    for criterion in criteria {
        let key = &criterion["key"];
        assert_eq!(
            criterion["rules"].as_array().unwrap(),
            &of("findings", key, "rule"),
            "{criterion}"
        );
        let names: Vec<Value> = criterion["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|test| test["name"].clone())
            .collect();
        assert_eq!(names, of("tests", key, "name"), "{criterion}");
    }
}

#[test]
fn a_trace_view_records_reconcile_s_counts_and_outcome_and_none_of_its_tests_or_findings() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let report = report(&yard, "trace");
    let recorded = &report["reconcile"];
    let reconcile = yard.loft(&["reconcile"]);
    assert!(recorded.get("tests").is_none(), "{recorded}");
    assert!(recorded.get("findings").is_none(), "{recorded}");
    for field in [
        "schema", "outcome", "inScope", "linked", "accepted", "errors",
    ] {
        assert_eq!(recorded[field], reconcile[field], "{field}: {recorded}");
    }
    assert!(
        report["counts"].get("reconcile.tests").is_none(),
        "{report}"
    );
    assert!(
        report["leftOut"].get("reconcile.findings").is_none(),
        "{report}"
    );
}

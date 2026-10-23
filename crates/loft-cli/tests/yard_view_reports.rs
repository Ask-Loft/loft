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
    let read = yard.read("s1", &[]);
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

#[test]
fn a_spec_change_view_draws_a_changed_feature_under_its_feature_group_and_product() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let graph = report(&yard, "spec-change")["graph"].clone();
    let feature = node(&graph, "Share a list");
    assert_eq!(feature["kind"], "feature");
    assert_eq!(feature["change"], "changed");
    let group = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["key"] == feature["parent"])
        .expect("the Feature Group above the Feature");
    assert_eq!(group["kind"], "feature_group");
    assert_eq!(group["change"], Value::Null, "drawn only for context");
    let product = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["key"] == group["parent"])
        .expect("the Product above the Feature Group");
    assert_eq!(product["kind"], "product");
    assert_eq!(product["change"], Value::Null);
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 3, "{graph}");
}

#[test]
fn a_spec_change_view_counts_a_feature_s_requirements_and_criteria_and_those_changed() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let graph = report(&yard, "spec-change")["graph"].clone();
    let feature = node(&graph, "Share a list");
    assert!(feature["requirements"].as_u64().unwrap() > 0, "{feature}");
    assert!(feature["criteria"].as_u64().unwrap() > 0, "{feature}");
    assert_eq!(feature["delta"]["criteria"]["changed"], 1, "{feature}");
    assert_eq!(feature["delta"]["criteria"]["added"], 0, "{feature}");
    assert_eq!(feature["delta"]["requirements"]["changed"], 0, "{feature}");
    assert!(
        feature["label"].is_string(),
        "a published node keeps its label"
    );
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

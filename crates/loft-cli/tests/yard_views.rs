//! The `loft` views a `loft` block records, as processes: each computed by this `loft`'s own
//! library over the tree that holds Yard Log, holding the document the matching command prints
//! for the same tree, with where and when it was computed; a view that cannot be computed
//! refuses the batch and changes no file; and a long list is kept to its first 1,000 items.

use std::fmt::Write as _;
use std::fs;

use serde_json::{Value, json};

use crate::common::copies::{HANDLE, copy_lookup};
use crate::interfaces::{LOOKUP, SHELF_STORE, add_lookup};
use crate::snapshot::snapshot;
use crate::yard::{EPOCH_TEXT, HEAD, Yard, block, document};

/// The feature file the tests change.
const SHARE: &str = "products/reading-list/sharing/share-a-list.yaml";

/// A batch of a stage and a `loft` block `v1` of `view`, with `extra` fields.
fn batch(view: &str, extra: &str) -> String {
    format!(
        r#"[{{"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"}},
            {{"id": "v1", "kind": "loft", "stage": "shape", "view": "{view}"{extra}}}]"#
    )
}

/// Records the view `view` with `extra` fields, and gives the `loft` block's current body.
fn recorded(yard: &Yard, view: &str, extra: &str) -> Value {
    let written = yard.write("s1", &batch(view, extra), &[]);
    assert_eq!(written["outcome"], "written", "{written}");
    let read = yard.read("s1", &["--reports"]);
    block(&read, "v1")["version"]["body"].clone()
}

/// Changes one criterion of the tree, so that it differs from its base.
fn change_a_criterion(yard: &Yard) {
    yard.scratch.edit(
        SHARE,
        "An edit made through the link is refused.",
        "An edit made through the link is refused, and the link says so.",
    );
}

#[test]
fn a_spec_change_view_holds_what_loft_status_prints_against_the_base_branch() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let body = recorded(&yard, "spec-change", "");
    let status = yard.loft(&["status", "--base", "develop"]);
    assert_eq!(body["report"]["status"], status);
    assert_eq!(
        status["base"]["changed"][0]["path"],
        format!("spec/{SHARE}")
    );
    let changed = status["base"]["changed"].as_array().unwrap().len();
    assert_eq!(body["report"]["counts"]["status.base.changed"], changed);
}

#[test]
fn a_view_compares_with_the_base_revision_the_block_names() {
    let yard = Yard::with_spec();
    // The command's base branch names no commit the repository holds; the block's base does.
    let named = batch("spec-change", r#", "base": "develop""#);
    let written = yard.write("s1", &named, &["--base", "main"]);
    assert_eq!(written["outcome"], "written", "{written}");
    let read = yard.read("s1", &["--reports"]);
    let body = &block(&read, "v1")["version"]["body"];
    assert_eq!(body["base"], "develop");
    assert_eq!(body["report"]["status"]["base"]["revision"], "develop");
}

#[test]
fn a_block_records_the_commit_branch_uncommitted_changes_loft_version_and_time_of_its_report() {
    let mut yard = Yard::with_spec();
    yard.git = yard.git.clone().with(" M", &format!("spec/{SHARE}"));
    let body = recorded(&yard, "publish", "");
    assert_eq!(
        body["recorded"],
        json!({
            "commit": HEAD,
            "branch": "feature",
            "dirty": true,
            "loft": env!("CARGO_PKG_VERSION"),
            "at": EPOCH_TEXT,
        })
    );
}

#[test]
fn a_findings_view_holds_what_loft_check_prints_against_the_base_branch() {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let body = recorded(&yard, "findings", "");
    let check = yard.loft(&["check", "--baseline", "develop"]);
    assert_eq!(body["report"]["check"], check);
}

#[test]
fn a_findings_view_keeps_the_agent_s_reason_for_each_finding_it_leaves() {
    let yard = Yard::with_spec();
    let reasons = r#", "reasons": [{"rule": "LQ-005", "key": "00000000-0000-4000-8000-000000000036",
        "reason": "one rule, stated once"}]"#;
    let body = recorded(&yard, "findings", reasons);
    assert_eq!(body["reasons"][0]["reason"], "one rule, stated once");
    assert_eq!(body["reasons"][0]["rule"], "LQ-005");
}

#[test]
fn a_trace_view_holds_the_outcome_of_loft_reconcile_and_the_criteria_of_each_feature_the_branch_changed()
 {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let body = recorded(&yard, "trace", "");
    let reconcile = yard.loft(&["reconcile"]);
    assert_eq!(body["report"]["reconcile"]["outcome"], reconcile["outcome"]);
    assert_eq!(body["report"]["base"], "develop");
    let features = body["report"]["features"].as_array().unwrap();
    assert_eq!(features.len(), 1, "{features:?}");
    assert_eq!(features[0]["name"], "Share a list");
    let criteria = features[0]["criteria"].as_array().unwrap();
    assert!(
        criteria
            .iter()
            .any(|criterion| criterion["key"] == "00000000-0000-4000-8000-000000000036")
    );
}

#[test]
fn a_publish_view_holds_what_loft_status_prints() {
    let yard = Yard::with_spec();
    let body = recorded(&yard, "publish", "");
    assert_eq!(body["report"]["status"], yard.loft(&["status"]));
}

#[test]
fn a_decision_view_holds_the_decision_its_block_names_by_slug_key_or_label() {
    let yard = Yard::with_spec();
    let body = recorded(
        &yard,
        "decision",
        r#", "decision": "store-a-shelf-in-one-file""#,
    );
    let decision = &body["report"]["decision"];
    assert_eq!(decision["slug"], "store-a-shelf-in-one-file");
    let found = yard.loft(&["find", "store-a-shelf-in-one-file"]);
    assert_eq!(decision["key"], found["matches"][0]["key"]);
    assert_eq!(decision["label"], found["matches"][0]["label"]);
    assert_eq!(decision["title"], found["matches"][0]["name"]);
    for field in [
        "context",
        "state",
        "drivers",
        "options",
        "assessments",
        "consequences",
    ] {
        assert!(decision.get(field).is_some(), "{field}");
    }
    let subjects = decision["subjects"].as_array().unwrap();
    for subject in subjects {
        assert!(subject["node"]["name"].is_string(), "{subject}");
    }
}

#[test]
fn a_view_that_cannot_be_computed_refuses_the_batch_naming_block_view_and_error() {
    let yard = Yard::new();
    let before = snapshot(&yard.scratch.0);
    let output = yard.run_with(&["write", "--session", "s1"], &batch("spec-change", ""));
    let refused = document("write", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(refused["errors"][0]["kind"], "view");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("`spec-change`"), "{message}");
    assert!(message.contains("`v1`"), "{message}");
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

/// The criteria `count` new criteria of Share a list, as its file writes them, and their keys.
fn many_criteria(count: usize) -> (String, Vec<String>) {
    let mut written = String::new();
    let mut keys = Vec::new();
    for number in 0..count {
        let key = format!("00000000-0000-4000-9000-{number:012x}");
        let _ = write!(
            written,
            "\n      - key: {key}\n        name: \"Criterion {number} holds.\""
        );
        keys.push(key);
    }
    (written, keys)
}

/// The last criterion of Share a list, after which [`many_criteria`] are written.
const LAST: &str = "        name: \"An edit made through the link is refused.\"";

#[test]
fn a_view_of_more_than_1000_items_records_every_one_and_counts_them() {
    let yard = Yard::with_spec();
    let (criteria, _) = many_criteria(1_001);
    yard.scratch.edit(SHARE, LAST, &format!("{LAST}{criteria}"));
    let report = &recorded(&yard, "spec-change", "")["report"];
    let added = report["status"]["base"]["added"].as_array().unwrap();
    assert_eq!(added.len(), 1_001);
    assert_eq!(report["graph"]["diff"].as_array().unwrap().len(), 1_001);
    assert_eq!(report["counts"]["status.base.added"], 1_001);
    assert_eq!(
        report.get("leftOut"),
        None,
        "nothing is left out: {}",
        report["counts"]
    );
}

#[test]
fn a_view_records_every_subject_a_decision_gained() {
    let mut yard = Yard::with_spec();
    let (criteria, keys) = many_criteria(1_001);
    yard.scratch.edit(SHARE, LAST, &format!("{LAST}{criteria}"));
    yard.rebased();
    let mut subjects = String::from("subjects:\n");
    for key in &keys {
        let _ = write!(subjects, "  - role: affected\n    node: {key}\n");
    }
    yard.scratch
        .edit("decisions/keep-a-change-log.yaml", "subjects:\n", &subjects);
    let report = &recorded(&yard, "spec-change", "")["report"];
    let entry = report["graph"]["diff"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "Keep a change log")
        .unwrap();
    assert_eq!(entry["subjects"].as_array().unwrap().len(), 1_001);
}

#[test]
fn a_trace_view_records_every_criterion_of_its_features_with_its_tests_and_rules() {
    let yard = Yard::with_spec();
    let (criteria, _) = many_criteria(1_001);
    yard.scratch.edit(SHARE, LAST, &format!("{LAST}{criteria}"));
    let report = &recorded(&yard, "trace", "")["report"];
    let recorded: Vec<&Value> = report["features"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|feature| feature["criteria"].as_array().unwrap())
        .collect();
    assert!(recorded.len() > 1_001, "{}", recorded.len());
    assert!(
        recorded
            .iter()
            .all(|criterion| criterion["tests"].is_array() && criterion["rules"].is_array())
    );
}

/// Runs `loft` with `args` in the folder `pkg/` of `yard`'s tree, printing JSON.
fn in_package(yard: &Yard, args: &[&str]) -> Value {
    let output = yard
        .scratch
        .command(Some(&yard.git), &yard.clock)
        .current_dir(yard.scratch.0.join("pkg"))
        .args(args)
        .args(["--format", "json"])
        .output()
        .unwrap();
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn a_view_is_computed_over_the_specification_below_the_top_of_the_worktree() {
    let mut yard = Yard::with_spec();
    let package = yard.scratch.0.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::rename(yard.scratch.0.join("spec"), package.join("spec")).unwrap();
    yard.git = yard.git.clone().below("pkg/");
    let input = yard.scratch.home().join("input.json");
    let named = r#", "decision": "store-a-shelf-in-one-file""#;
    fs::write(&input, batch("decision", named)).unwrap();
    let input = input.to_str().unwrap();
    let written = in_package(
        &yard,
        &["yard", "write", "--session", "s1", "--input", input],
    );
    assert_eq!(written["outcome"], "written", "{written}");
    assert!(yard.folder("feature").join("yardlog.yaml").is_file());
    let read = in_package(&yard, &["yard", "read", "--session", "s1", "--reports"]);
    let found = in_package(&yard, &["find", "store-a-shelf-in-one-file"]);
    assert_eq!(
        block(&read, "v1")["version"]["body"]["report"]["decision"]["key"],
        found["matches"][0]["key"]
    );
}

#[test]
fn a_spec_change_view_draws_an_added_interface_and_the_provides_and_requires_edges_to_it() {
    let yard = Yard::with_spec();
    add_lookup(&yard.scratch);
    let link_service = "00000000-0000-4000-8000-000000000041";
    yard.scratch.edit(
        "components/link-service.yaml",
        "boundary: external\n",
        &format!("boundary: external\nrequires:\n  - {LOOKUP}\n"),
    );
    let graph = &recorded(&yard, "spec-change", "")["report"]["graph"];
    let node = |key: &str| {
        graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["key"] == key)
            .unwrap_or_else(|| panic!("no {key} in {graph}"))
            .clone()
    };
    assert_eq!(
        [node(LOOKUP)["kind"].clone(), node(LOOKUP)["change"].clone()],
        [json!("interface"), json!("added")]
    );
    for component in [SHELF_STORE, link_service] {
        assert_eq!(node(component)["change"], "changed", "{component}");
    }
    let edges: Vec<&Value> = graph["edges"].as_array().unwrap().iter().collect();
    assert!(
        edges.contains(&&json!({ "from": SHELF_STORE, "to": LOOKUP, "kind": "provides" })),
        "{edges:?}"
    );
    assert!(
        edges.contains(&&json!({ "from": link_service, "to": LOOKUP, "kind": "requires" })),
        "{edges:?}"
    );
}

#[test]
fn a_spec_change_view_leaves_out_an_edge_to_a_node_of_another_documents_copy() {
    let yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    yard.scratch.edit(
        "components/shelf-store.yaml",
        "scopedTo:\n",
        &format!("requires:\n  - document: {HANDLE}\n    key: {LOOKUP}\nscopedTo:\n"),
    );
    let report = recorded(&yard, "spec-change", "");
    let graph = &report["report"]["graph"];
    let keys: Vec<&Value> = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| &node["key"])
        .collect();
    assert!(!keys.contains(&&json!(LOOKUP)), "{graph}");
    let changed = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["key"] == SHELF_STORE);
    assert_eq!(changed.unwrap()["change"], "changed", "{graph}");
    let edges = graph["edges"].as_array().unwrap();
    assert!(edges.iter().all(|edge| edge["to"] != LOOKUP), "{edges:?}");
}

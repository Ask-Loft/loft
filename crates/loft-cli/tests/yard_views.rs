//! The `loft` views a `loft` block records, as processes: each computed by this `loft`'s own
//! library over the tree that holds Yard Log, holding the document the matching command prints
//! for the same tree, with where and when it was computed; a view that cannot be computed
//! refuses the batch and changes no file; and a long list is kept to its first 1,000 items.

use std::fmt::Write as _;

use serde_json::{Value, json};

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
    let read = yard.read("s1", &[]);
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
    let read = yard.read("s1", &["--base", "main"]);
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
fn a_trace_view_holds_what_loft_reconcile_prints_and_the_criteria_of_each_feature_the_branch_changed()
 {
    let yard = Yard::with_spec();
    change_a_criterion(&yard);
    let body = recorded(&yard, "trace", "");
    assert_eq!(body["report"]["reconcile"], yard.loft(&["reconcile"]));
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

#[test]
fn a_view_of_more_than_1000_items_keeps_the_first_1000_and_counts_the_whole() {
    let yard = Yard::with_spec();
    let mut added = String::new();
    for number in 0..1_001 {
        let _ = write!(
            added,
            "\n      - key: 00000000-0000-4000-9000-{number:012x}\n        name: \"Criterion {number} holds.\""
        );
    }
    let last = "        name: \"An edit made through the link is refused.\"";
    yard.scratch.edit(SHARE, last, &format!("{last}{added}"));
    let body = recorded(&yard, "spec-change", "");
    let report = &body["report"];
    assert_eq!(
        report["status"]["base"]["added"].as_array().unwrap().len(),
        1_000
    );
    assert_eq!(report["counts"]["status.base.added"], 1_001);
    assert_eq!(report["leftOut"]["status.base.added"], 1);
}

#[test]
fn a_view_is_computed_over_the_specification_below_the_top_of_the_worktree() {
    let mut yard = Yard::with_spec();
    yard.git = yard.git.clone().below("pkg/");
    let named = r#", "decision": "store-a-shelf-in-one-file""#;
    let body = recorded(&yard, "decision", named);
    let found = yard.loft(&["find", "store-a-shelf-in-one-file"]);
    assert_eq!(
        body["report"]["decision"]["key"],
        found["matches"][0]["key"]
    );
}

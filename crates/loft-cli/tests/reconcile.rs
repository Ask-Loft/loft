//! `loft reconcile` (the `loft reconcile` Feature) over the Reading List, whose nodes in scope
//! are its seven criteria and the proved Constraint CON-1: each finding on one line, the counts
//! at the end, `--fail-on`, accepting links, and the trace file's errors.

use std::fs;
use std::process::Output;

use crate::common::{Scratch, stderr, stdout};
use crate::schema;
use crate::snapshot::snapshot;
use serde_json::Value;

const AC_1: &str = "00000000-0000-4000-8000-000000000029";
const AC_2: &str = "00000000-0000-4000-8000-00000000002a";
const ADD_A_BOOK: &str = "products/reading-list/shelf/add-a-book.yaml";
const TEST_FILE: &str = "tests/shelf.rs";

/// The Reading List with `tests/shelf.rs` holding the test `a_book_is_added`.
fn project() -> Scratch {
    let scratch = Scratch::with_reading_list();
    fs::create_dir_all(scratch.0.join("tests")).unwrap();
    fs::write(scratch.0.join(TEST_FILE), "fn a_book_is_added() {}\n").unwrap();
    scratch
}

/// A link to `node` verified by `name` in `tests/shelf.rs`.
fn link(node: &str, name: &str) -> String {
    format!("  - node: {node}\n    tests:\n      - file: {TEST_FILE}\n        name: {name}\n")
}

fn write_trace(scratch: &Scratch, links: &[String]) {
    let text = format!("format: loft-trace/1\nlinks:\n{}", links.concat());
    fs::write(scratch.0.join("loft.trace.yaml"), text).unwrap();
}

fn trace_text(scratch: &Scratch) -> String {
    fs::read_to_string(scratch.0.join("loft.trace.yaml")).unwrap()
}

/// The rule and label of each finding line, in order.
fn findings(output: &Output) -> Vec<(String, String)> {
    stdout(output)
        .lines()
        .filter_map(|line| {
            let (_, rest) = line
                .split_once(": info ")
                .or(line.split_once(": warning "))?;
            let (rule, rest) = rest.split_once(": ")?;
            let (label, _) = rest.split_once(": ")?;
            Some((rule.to_owned(), label.to_owned()))
        })
        .collect()
}

fn json_of(output: &Output) -> Value {
    schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/reconcile.json"),
    )
}

#[test]
fn without_a_trace_file_each_node_in_scope_is_reported_unlinked_and_nothing_is_written() {
    let scratch = project();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["reconcile"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let found = findings(&output);
    assert_eq!(found.len(), 8, "{}", stdout(&output));
    assert!(found.iter().all(|(rule, _)| rule == "TR-001"));
    let first = stdout(&output).lines().next().unwrap().to_owned();
    assert_eq!(
        first,
        format!("spec/{ADD_A_BOOK}:19:14: info TR-001: AC-1: no link names this node")
    );
    let last = stdout(&output).lines().last().unwrap().to_owned();
    assert_eq!(
        last,
        "0 of 8 nodes in scope linked; TR-001 8, TR-002 0, TR-003 0, TR-004 0, TR-005 0"
    );
    assert!(snapshot(&scratch.0) == before, "a file changed");
}

#[test]
fn a_node_without_a_label_is_printed_with_a_dash_and_null_in_json() {
    let scratch = project();
    scratch.edit(
        ADD_A_BOOK,
        "    criteria:\n",
        "    criteria:\n      - key: 01a13900-0000-7000-8000-000000000001\n        name: \"A new outcome holds.\"\n",
    );
    let output = scratch.loft(&["reconcile"]);
    assert!(
        findings(&output).contains(&("TR-001".to_owned(), "-".to_owned())),
        "{}",
        stdout(&output)
    );
    let json = json_of(&scratch.loft(&["reconcile", "--format", "json"]));
    let unlabelled = json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["key"] == "01a13900-0000-7000-8000-000000000001")
        .unwrap();
    assert_eq!(unlabelled["label"], Value::Null);
}

#[test]
fn fail_on_warning_exits_1_on_an_unreviewed_link_and_0_on_only_info() {
    let scratch = project();
    assert_eq!(
        scratch
            .loft(&["reconcile", "--fail-on", "warning"])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(
        scratch
            .loft(&["reconcile", "--fail-on", "info"])
            .status
            .code(),
        Some(1)
    );
    write_trace(&scratch, &[link(AC_1, "a_book_is_added")]);
    let output = scratch.loft(&["reconcile"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        findings(&output)[0],
        ("TR-002".to_owned(), "AC-1".to_owned())
    );
    assert_eq!(
        scratch
            .loft(&["reconcile", "--fail-on", "warning"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        scratch
            .loft(&["reconcile", "--fail-on", "fatal"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn accepting_a_link_by_label_writes_its_review_and_reports_the_trace_file_as_written() {
    let scratch = project();
    write_trace(&scratch, &[link(AC_1, "a_book_is_added")]);
    let output = scratch.loft(&["reconcile", "--accept", "AC-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output).lines().next(), Some("accepted AC-1"));
    assert!(
        trace_text(&scratch).contains("reviewed: "),
        "{}",
        trace_text(&scratch)
    );
    assert!(
        !findings(&output).iter().any(|(_, label)| label == "AC-1"),
        "{}",
        stdout(&output)
    );
    let again = scratch.loft(&["reconcile", "--fail-on", "warning"]);
    assert_eq!(again.status.code(), Some(0), "{}", stdout(&again));
}

#[test]
fn accept_takes_keys_and_can_be_given_more_than_once() {
    let scratch = project();
    write_trace(
        &scratch,
        &[link(AC_2, "a_book_is_added"), link(AC_1, "a_book_is_added")],
    );
    let output = scratch.loft(&["reconcile", "--accept", AC_1, "--accept", "AC-2"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = trace_text(&scratch);
    assert_eq!(text.matches("reviewed: ").count(), 2, "{text}");
    // Written in document order: AC-1 before AC-2.
    assert!(
        text.find(AC_1).unwrap() < text.find(AC_2).unwrap(),
        "{text}"
    );
}

#[test]
fn a_node_that_cannot_be_accepted_writes_nothing_and_names_each_refusal() {
    let scratch = project();
    write_trace(
        &scratch,
        &[link(AC_1, "a_book_is_added"), link(AC_2, "not_there")],
    );
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&[
        "reconcile",
        "--accept",
        "AC-1",
        "--accept",
        "AC-2",
        "--accept",
        "AC-3",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let errors = stderr(&output);
    assert!(
        errors.contains("cannot accept AC-2: the link has a TR-004 finding"),
        "{errors}"
    );
    assert!(
        errors.contains("cannot accept AC-3: no link names this node"),
        "{errors}"
    );
    assert!(!errors.contains("cannot accept AC-1"), "{errors}");
    assert!(snapshot(&scratch.0) == before, "a file changed");
    let unknown = scratch.loft(&["reconcile", "--accept", "AC-99"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(stderr(&unknown).contains("AC-99"), "{}", stderr(&unknown));
    let json = json_of(&scratch.loft(&["reconcile", "--accept", "AC-3", "--format", "json"]));
    assert_eq!(json["outcome"], "error");
    assert_eq!(json["errors"][0]["kind"], "accept");
}

#[test]
fn accepting_the_stale_links_leaves_a_link_whose_test_is_missing() {
    let scratch = project();
    write_trace(
        &scratch,
        &[link(AC_1, "a_book_is_added"), link(AC_2, "not_there")],
    );
    let output = scratch.loft(&["reconcile", "--accept-stale"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output).lines().next(), Some("accepted AC-1"));
    let found = findings(&output);
    assert!(
        found.contains(&("TR-002".to_owned(), "AC-2".to_owned())),
        "{found:?}"
    );
    assert!(
        found.contains(&("TR-004".to_owned(), "AC-2".to_owned())),
        "{found:?}"
    );
    let before = snapshot(&scratch.0);
    scratch.loft(&["reconcile", "--accept-stale"]);
    assert!(
        snapshot(&scratch.0) == before,
        "nothing to accept wrote a file"
    );
}

#[test]
fn a_link_to_a_key_no_node_carries_is_reported_at_its_line_in_the_trace_file() {
    let scratch = project();
    let unknown = "01a13900-0000-7000-8000-0000000000ff";
    write_trace(&scratch, &[link(unknown, "a_book_is_added")]);
    let output = scratch.loft(&["reconcile"]);
    let line = stdout(&output)
        .lines()
        .find(|line| line.contains("TR-005"))
        .unwrap()
        .to_owned();
    assert_eq!(
        line,
        "loft.trace.yaml:3:5: warning TR-005: -: no node of the tree carries this key"
    );
}

#[test]
fn a_broken_trace_file_is_an_error_at_its_line_with_no_finding_and_no_write() {
    let scratch = project();
    fs::write(
        scratch.0.join("loft.trace.yaml"),
        "format: loft-trace/1\nlinks:\nowner: me\n",
    )
    .unwrap();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["reconcile", "--accept-stale"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("loft.trace.yaml:3:1: "),
        "{}",
        stderr(&output)
    );
    assert_eq!(findings(&output), []);
    assert!(snapshot(&scratch.0) == before, "a file changed");
    let json = json_of(&scratch.loft(&["reconcile", "--format", "json"]));
    assert_eq!(json["outcome"], "error");
    assert_eq!(json["errors"][0]["kind"], "trace");
    assert_eq!(json["errors"][0]["line"], 3);
}

#[test]
fn the_json_document_carries_each_finding_the_counts_and_the_accepted_keys() {
    let scratch = project();
    write_trace(
        &scratch,
        &[link(AC_1, "a_book_is_added"), link(AC_2, "not_there")],
    );
    let json = json_of(&scratch.loft(&["reconcile", "--accept", "AC-1", "--format", "json"]));
    assert_eq!(json["outcome"], "passed");
    assert_eq!(json["accepted"], serde_json::json!([AC_1]));
    assert_eq!(
        (json["inScope"].as_u64(), json["linked"].as_u64()),
        (Some(8), Some(2))
    );
    let missing = json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["rule"] == "TR-004")
        .unwrap();
    assert_eq!(missing["label"], "AC-2");
    assert_eq!(missing["key"], AC_2);
    assert_eq!(missing["path"], format!("spec/{ADD_A_BOOK}"));
    assert_eq!(missing["test"]["name"], "not_there");
    assert_eq!(missing["severity"], "warning");
    let failed = json_of(&scratch.loft(&["reconcile", "--fail-on", "warning", "--format", "json"]));
    assert_eq!(failed["outcome"], "failed");
}

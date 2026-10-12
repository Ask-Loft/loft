//! `loft check` as a process (D-11 F-38, F-44, F-45, F-47, RQ-174): what it prints, in which
//! order and form, and the exit code a CI job gates on.

mod common;
#[path = "common/schema.rs"]
mod schema;

use std::fs;

use common::{Scratch, stderr, stdout};
use serde_json::Value;

const PRODUCT: &str = "products/reading-list/product.yaml";
const SHARE: &str = "products/reading-list/sharing/share-a-list.yaml";

/// Disables every rule that reports on the Reading List, so the tree checks clean.
const QUIET: &str = "rules:\n  - rule: SH-002\n    enabled: false\n  - rule: SH-008\n    \
                     enabled: false\n  - rule: V2-001\n    enabled: false\n  - rule: V2-002\n    \
                     enabled: false\n  - rule: V2-003\n    enabled: false\n";

/// The Reading List with one tag listed twice in each of two files.
fn with_two_errors() -> Scratch {
    let scratch = Scratch::with_reading_list();
    scratch.edit(PRODUCT, "  - portable\n", "  - portable\n  - portable\n");
    scratch.edit(
        SHARE,
        "      - portable\n",
        "      - portable\n      - portable\n",
    );
    scratch
}

fn json(output: &std::process::Output) -> Value {
    let document: Value = serde_json::from_str(&stdout(output)).unwrap();
    let schema: Value = serde_json::from_str(include_str!("../../../schemas/check.json")).unwrap();
    let violations = schema::violations(&document, &schema);
    assert!(violations.is_empty(), "{violations:#?}");
    document
}

#[test]
fn ac_526_ac_636_a_clean_tree_exits_0_and_prints_nothing() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.file("lint.yaml"), QUIET).unwrap();
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "");
}

#[test]
fn ac_525_findings_alone_exit_0_one_per_line() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(0));
    let lines: Vec<String> = stdout(&output).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 6, "{lines:#?}");
    assert!(lines[0].starts_with("spec/components/shared-types.yaml:1:6: info SH-002: "));
}

#[test]
fn ac_522_ac_523_ac_524_every_validation_error_prints_before_the_findings_and_exits_1() {
    let scratch = with_two_errors();
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[0].starts_with(&format!("spec/{PRODUCT}:1:6: ")),
        "{text}"
    );
    assert!(
        lines[0].ends_with("lists the tag `portable` twice"),
        "{text}"
    );
    assert!(lines[1].starts_with(&format!("spec/{SHARE}:")), "{text}");
    assert!(
        lines[2..]
            .iter()
            .all(|line| line.contains(" SH-") || line.contains(" V2-"))
    );
}

#[test]
fn ac_528_ac_592_a_read_error_is_reported_at_its_position_and_lint_does_not_run() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(PRODUCT, "\nslug:", " # a note\nslug:");
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.starts_with(&format!("spec/{PRODUCT}:1:")), "{text}");
}

#[test]
fn ac_529_a_directory_without_spec_is_refused_naming_the_path() {
    let scratch = Scratch::with_reading_list();
    fs::remove_dir_all(scratch.file("")).unwrap();
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("./spec: "),
        "{}",
        stderr(&output)
    );
}

#[test]
fn ac_530_ac_638_fail_on_warning_or_info_exits_1_on_a_warning() {
    let scratch = Scratch::with_reading_list();
    let warning = "rules:\n  - rule: V2-002\n    severity: warning\n";
    fs::write(scratch.file("lint.yaml"), warning).unwrap();
    for severity in ["warning", "info"] {
        assert_eq!(
            scratch
                .loft(&["check", "--fail-on", severity])
                .status
                .code(),
            Some(1)
        );
    }
}

#[test]
fn ac_531_ac_532_ac_639_fail_on_counts_only_findings_at_or_above_it() {
    let scratch = Scratch::with_reading_list();
    let infos = "rules:\n  - rule: V2-003\n    enabled: false\n";
    fs::write(scratch.file("lint.yaml"), infos).unwrap();
    assert_eq!(
        scratch
            .loft(&["check", "--fail-on", "warning"])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(
        scratch.loft(&["check", "--fail-on", "info"]).status.code(),
        Some(1)
    );
    assert_eq!(
        scratch.loft(&["check", "--fail-on", "hint"]).status.code(),
        Some(1)
    );
}

#[test]
fn ac_533_a_severity_outside_the_three_is_a_usage_error() {
    let scratch = Scratch::with_reading_list();
    assert_eq!(
        scratch.loft(&["check", "--fail-on", "error"]).status.code(),
        Some(2)
    );
}

#[test]
fn ac_582_ac_585_ac_593_json_matches_the_schema_with_positions_and_the_same_exit_code() {
    let scratch = with_two_errors();
    let output = scratch.loft(&["check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document = json(&output);
    let error = &document["errors"][0];
    assert_eq!(error["kind"], "validation");
    assert_eq!(error["path"], format!("spec/{PRODUCT}"));
    assert_eq!(
        (error["line"].as_u64(), error["column"].as_u64()),
        (Some(1), Some(6))
    );
    assert!(!document["findings"].as_array().unwrap().is_empty());
}

#[test]
fn ac_583_ac_584_ac_594_an_error_without_a_position_is_json_on_stdout() {
    let scratch = Scratch::with_reading_list();
    fs::remove_dir_all(scratch.file("")).unwrap();
    let output = scratch.loft(&["check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document = json(&output);
    let error = &document["errors"][0];
    assert!(error.get("path").is_none() && error.get("line").is_none());
}

#[test]
fn ac_582_a_clean_tree_in_json_has_no_errors_and_no_findings() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.file("lint.yaml"), QUIET).unwrap();
    let output = scratch.loft(&["check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let document = json(&output);
    assert_eq!(document["errors"], Value::Array(Vec::new()));
    assert_eq!(document["findings"], Value::Array(Vec::new()));
}

#[test]
fn ac_672_ac_674_an_unknown_rule_in_the_lint_file_is_an_error_and_no_finding_is_reported() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.file("lint.yaml"), "rules:\n  - rule: XX-001\n").unwrap();
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stdout(&output),
        "spec/lint.yaml:2:11: `XX-001` is not a rule the Format defines\n"
    );
}

#[test]
fn entries_whose_names_begin_with_a_dot_are_not_part_of_the_tree() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.file(".DS_Store"), [0, 0, 0, 1]).unwrap();
    fs::write(scratch.file("products/.DS_Store"), [0, 0, 0, 1]).unwrap();
    fs::create_dir(scratch.file(".hidden")).unwrap();
    fs::write(scratch.file(".hidden/notes.yaml"), "not: [a, record]\n").unwrap();
    let output = scratch.loft(&["check", "--fail-on", "warning"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        !stdout(&output).contains(".DS_Store"),
        "{}",
        stdout(&output)
    );
    assert_eq!(scratch.loft(&["fmt", "--check"]).status.code(), Some(0));
}

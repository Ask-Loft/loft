//! `loft check` as a process (F-38, F-44, F-45, F-47, RQ-174): what it prints, in which
//! order and form, and the exit code a CI job gates on.

use std::fs;
use std::process::Command;

use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::schema;
use serde_json::Value;
use software_plant::Clock;

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
    schema::conforming(&output.stdout, include_str!("../../../schemas/check.json"))
}

#[test]
fn a_clean_tree_exits_0_and_prints_nothing() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.file("lint.yaml"), QUIET).unwrap();
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "");
}

#[test]
fn findings_alone_exit_0_one_per_line() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(0));
    let lines: Vec<String> = stdout(&output).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 6, "{lines:#?}");
    assert!(lines[0].starts_with("spec/components/shared-types.yaml:1:6: info SH-002: "));
}

#[test]
fn every_validation_error_prints_before_the_findings_and_exits_1() {
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
fn a_read_error_is_reported_at_its_position_and_lint_does_not_run() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(PRODUCT, "\nslug:", " # a note\nslug:");
    let output = scratch.loft(&["check"]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.starts_with(&format!("spec/{PRODUCT}:1:")), "{text}");
}

#[test]
fn a_directory_without_spec_is_refused_naming_the_path() {
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
fn fail_on_warning_or_info_exits_1_on_a_warning() {
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
fn fail_on_counts_only_findings_at_or_above_it() {
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
fn a_severity_outside_the_three_is_a_usage_error() {
    let scratch = Scratch::with_reading_list();
    assert_eq!(
        scratch.loft(&["check", "--fail-on", "error"]).status.code(),
        Some(2)
    );
}

#[test]
fn json_matches_the_schema_with_positions_and_the_same_exit_code() {
    let scratch = with_two_errors();
    let output = scratch.loft(&["check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document = json(&output);
    assert_eq!(document["outcome"], "failed");
    let error = &document["errors"][0];
    assert_eq!(error["kind"], "validation");
    assert_eq!(error["path"], format!("spec/{PRODUCT}"));
    assert_eq!(
        (error["line"].as_u64(), error["column"].as_u64()),
        (Some(1), Some(6))
    );
    assert_ne!(
        document["findings"].as_array().unwrap(),
        &Vec::<Value>::new()
    );
}

#[test]
fn an_error_without_a_position_is_json_on_stdout() {
    let scratch = Scratch::with_reading_list();
    fs::remove_dir_all(scratch.file("")).unwrap();
    let output = scratch.loft(&["check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document = json(&output);
    assert_eq!(document["outcome"], "error");
    assert!(document.get("findings").is_none());
    let error = &document["errors"][0];
    assert!(error.get("path").is_none() && error.get("line").is_none());
}

#[test]
fn a_finding_at_or_above_fail_on_is_a_failed_check_in_json() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["check", "--fail-on", "hint", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document = json(&output);
    assert_eq!(document["outcome"], "failed");
    assert_eq!(document["errors"], Value::Array(Vec::new()));
}

#[test]
fn a_clean_tree_in_json_has_no_errors_and_no_findings() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.file("lint.yaml"), QUIET).unwrap();
    let output = scratch.loft(&["check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let document = json(&output);
    assert_eq!(document["outcome"], "passed");
    assert_eq!(document["errors"], Value::Array(Vec::new()));
    assert_eq!(document["findings"], Value::Array(Vec::new()));
}

#[test]
fn an_unknown_rule_in_the_lint_file_is_an_error_and_no_finding_is_reported() {
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

/// The JSON report and the text report of `loft check` run by `command`, each as printed.
fn reports(command: impl Fn() -> Command) -> (String, String) {
    let run = |args: &[&str]| stdout(&command().args(args).output().unwrap());
    (run(&["check", "--format", "json"]), run(&["check"]))
}

/// The findings of a JSON report.
fn findings_of(report: &str) -> Vec<Value> {
    let document: Value = serde_json::from_str(report).unwrap();
    let findings = document["findings"].as_array().unwrap().clone();
    assert!(!findings.is_empty(), "the Reading List has findings");
    findings
}

#[test]
fn the_report_does_not_depend_on_the_locale() {
    let scratch = Scratch::with_reading_list();
    let under = |locale: &str| {
        reports(|| {
            let mut command = scratch.command(None, &Clock::fixed(EPOCH));
            command
                .env("LC_ALL", locale)
                .env("LANG", locale)
                .env("LC_CTYPE", locale)
                .env("LC_COLLATE", locale)
                .env("LC_MESSAGES", locale);
            command
        })
    };
    let plain = under("C");
    findings_of(&plain.0);
    for locale in ["tr_TR.UTF-8", "de_DE.UTF-8", "ja_JP.UTF-8", "en_US.UTF-8"] {
        assert_eq!(under(locale), plain, "{locale}");
    }
}

#[test]
fn the_report_does_not_depend_on_environment_variables() {
    let scratch = Scratch::with_reading_list();
    let bare = reports(|| {
        let mut command = scratch.command(None, &Clock::fixed(EPOCH));
        command.env_clear();
        Clock::fixed(EPOCH).apply(&mut command);
        command
    });
    findings_of(&bare.0);
    let busy = reports(|| {
        let mut command = scratch.command(None, &Clock::fixed(EPOCH));
        command
            .env("LOFT_CONFIG", "lint.yaml")
            .env("LOFT_LINT", "off")
            .env("LOFT_FAIL_ON", "hint")
            .env("LOFT_FORMAT", "json")
            .env("NO_COLOR", "1")
            .env("CLICOLOR_FORCE", "1")
            .env("TERM", "xterm-256color")
            .env("COLUMNS", "20")
            .env("TZ", "Pacific/Kiritimati")
            .env("RUST_LOG", "trace")
            .env("HOME", scratch.0.join("home"));
        command
    });
    assert_eq!(busy, bare);
}

#[test]
fn no_command_line_option_changes_the_findings() {
    let scratch = Scratch::with_reading_list();
    let run = |args: &[&str]| stdout(&scratch.loft(args));
    let json = findings_of(&run(&["check", "--format", "json"]));
    let text = run(&["check"]);
    for severity in ["warning", "info", "hint"] {
        let failing = run(&["check", "--fail-on", severity, "--format", "json"]);
        assert_eq!(findings_of(&failing), json, "{severity}");
        assert_eq!(run(&["check", "--fail-on", severity]), text, "{severity}");
    }
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), json.len(), "{text}");
    for (line, finding) in lines.iter().zip(&json) {
        let said = format!(
            "{} {}: {}",
            finding["severity"].as_str().unwrap(),
            finding["rule"].as_str().unwrap(),
            finding["message"].as_str().unwrap()
        );
        assert!(line.contains(&said), "{line} does not say {said}");
    }
}

#[test]
fn no_file_outside_the_tree_changes_the_findings() {
    let scratch = Scratch::with_reading_list();
    let project = scratch.0.join("project");
    let home = scratch.0.join("home");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::rename(scratch.0.join("spec"), project.join("spec")).unwrap();
    let run = || {
        reports(|| {
            let mut command = scratch.command(None, &Clock::fixed(EPOCH));
            command
                .current_dir(&project)
                .env("HOME", &home)
                .env("XDG_CONFIG_HOME", home.join(".config"));
            command
        })
    };
    let before = run();
    findings_of(&before.0);
    for outside in [
        project.join("lint.yaml"),
        scratch.0.join("lint.yaml"),
        scratch.0.join("spec/lint.yaml"),
        home.join("lint.yaml"),
        home.join("spec/lint.yaml"),
        home.join(".config/loft/lint.yaml"),
    ] {
        fs::create_dir_all(outside.parent().unwrap()).unwrap();
        fs::write(&outside, QUIET).unwrap();
    }
    assert_eq!(run(), before);
}

#[test]
fn a_read_error_or_a_lint_configuration_error_is_a_failed_check_in_json() {
    let read = Scratch::with_reading_list();
    read.edit(PRODUCT, "\nslug:", " # a note\nslug:");
    let lint = Scratch::with_reading_list();
    fs::write(lint.file("lint.yaml"), "rules:\n  - rule: XX-001\n").unwrap();
    for (scratch, kind) in [(read, "read"), (lint, "lint")] {
        let output = scratch.loft(&["check", "--format", "json"]);
        assert_eq!(output.status.code(), Some(1), "{kind}");
        let document = json(&output);
        assert_eq!(document["outcome"], "failed", "{kind}: {document}");
        assert_ne!(document["errors"], Value::Array(Vec::new()), "{kind}");
    }
}

//! `loft render --openspec <dir>` as a process: the main specs and render map it writes, with
//! or without the other projections, the findings it prints, and the directories it refuses or,
//! with `--force`, replaces.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::{Scratch, stderr, stdout};
use crate::schema;
use crate::snapshot::snapshot;
use serde_json::Value;

/// The `OpenSpec` directory of a fixture project under `testdata/openspec/`.
fn fixture(project: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/openspec")
        .join(project)
        .join("openspec")
}

/// A scratch directory whose tree is the faithful fixture project, imported.
fn imported() -> Scratch {
    let scratch = Scratch::empty();
    let dir = fixture("faithful");
    let output = scratch.loft(&["import", "openspec", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch
}

fn put(scratch: &Scratch, path: &str, text: &str) {
    let target = scratch.0.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, text).unwrap();
}

fn read(scratch: &Scratch, path: &str) -> String {
    fs::read_to_string(scratch.0.join(path)).unwrap()
}

#[test]
fn openspec_alone_writes_the_main_specs_and_the_render_map_and_no_other_projection() {
    let scratch = imported();
    let output = scratch.loft(&["render", "--openspec", "out"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let original = fs::read_to_string(fixture("faithful").join("specs/auth/spec.md")).unwrap();
    assert_eq!(read(&scratch, "out/specs/auth/spec.md"), original);
    let map: Value = serde_json::from_str(&read(&scratch, "out/loft-render-map.json")).unwrap();
    assert_eq!(map["schema"], "loft-render-map/1");
    assert!(!scratch.0.join("docs").exists());
    assert_eq!(
        stdout(&output),
        "out/specs/auth/spec.md\nout/specs/billing/invoices/spec.md\nout/loft-render-map.json\n"
    );
}

#[test]
fn openspec_with_markdown_writes_both_and_not_the_json_export() {
    let scratch = imported();
    let output = scratch.loft(&["render", "--openspec", "out", "--markdown", "docs/a.md"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(scratch.0.join("out/specs/auth/spec.md").exists());
    assert!(scratch.0.join("docs/a.md").exists());
    assert!(!scratch.0.join("docs/spec.json").exists());
}

#[test]
fn each_finding_is_printed_on_its_own_line_and_none_prevents_the_render() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["render", "--openspec", "out"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    let findings: Vec<&str> = text.lines().filter(|line| line.contains(" OR-0")).collect();
    assert!(
        findings.iter().any(|line| line.starts_with("out/specs/")
            && line.contains(": info OR-003: the Purpose is shorter than 50 characters")),
        "{text}"
    );
    assert!(
        findings.iter().any(|line| line
            .starts_with("info OR-005: content OpenSpec has no place for was left out: ")),
        "{text}"
    );
    assert!(scratch.0.join("out/loft-render-map.json").exists());
}

#[test]
fn an_openspec_directory_under_spec_is_refused_and_nothing_is_written() {
    let scratch = imported();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&[
        "render",
        "--openspec",
        "spec/out",
        "--markdown",
        "docs/a.md",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("spec/out"), "{}", stderr(&output));
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn a_main_specs_directory_that_is_not_empty_is_refused_by_name_and_nothing_is_written() {
    let scratch = imported();
    put(&scratch, "out/specs/kept.txt", "kept\n");
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["render", "--openspec", "out", "--markdown", "docs/a.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("out/specs"), "{}", stderr(&output));
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn a_hidden_entry_does_not_make_the_main_specs_directory_not_empty() {
    let scratch = imported();
    put(&scratch, "out/specs/.DS_Store", "");
    let output = scratch.loft(&["render", "--openspec", "out"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

#[test]
fn force_removes_each_spec_it_does_not_write_and_changes_nothing_else() {
    let scratch = imported();
    put(&scratch, "out/specs/old/spec.md", "# Old\n");
    put(&scratch, "out/specs/old/design.md", "# Design\n");
    put(&scratch, "out/specs/auth/spec.md", "# Stale\n");
    put(&scratch, "out/specs/gone/spec.md", "# Gone\n");
    put(&scratch, "out/project.md", "# Project\n");
    let output = scratch.loft(&["render", "--openspec", "out", "--force", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = schema::conforming(&output.stdout, include_str!("../../../schemas/render.json"));
    assert_eq!(
        document["removed"],
        serde_json::json!(["out/specs/gone/spec.md", "out/specs/old/spec.md"])
    );
    assert!(!scratch.0.join("out/specs/old/spec.md").exists());
    assert!(!scratch.0.join("out/specs/gone").exists());
    assert_eq!(read(&scratch, "out/specs/old/design.md"), "# Design\n");
    assert_eq!(read(&scratch, "out/project.md"), "# Project\n");
    let original = fs::read_to_string(fixture("faithful").join("specs/auth/spec.md")).unwrap();
    assert_eq!(read(&scratch, "out/specs/auth/spec.md"), original);
}

#[test]
fn json_output_lists_the_files_written_and_each_finding() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["render", "--openspec", "out", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = schema::conforming(&output.stdout, include_str!("../../../schemas/render.json"));
    assert_eq!(document["outcome"], "rendered");
    let written = document["written"].as_array().unwrap();
    assert!(
        written.contains(&"out/loft-render-map.json".into()),
        "{written:?}"
    );
    let findings = document["findings"].as_array().unwrap();
    assert!(findings.iter().any(|finding| finding["rule"] == "OR-005"));
    assert!(findings.iter().all(|finding| finding["severity"] == "info"));
}

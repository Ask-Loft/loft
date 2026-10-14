//! `loft render` as a process (F-40, F-26): what it writes and where, what it refuses, and
//! that no projection feeds back into the tree.

use std::collections::BTreeMap;
use std::fs;

use std::process::{Command, Output};

use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::schema;
use serde_json::Value;
use software_plant::Clock;

/// Every file under `spec/` with its bytes.
fn tree(scratch: &Scratch) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![scratch.file("")];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.display().to_string(), fs::read(&path).unwrap());
            }
        }
    }
    files
}

/// The JSON document a render printed, held to the render schema.
fn json_of(output: &Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/render.json"))
}

fn exists(scratch: &Scratch, path: &str) -> bool {
    scratch.0.join(path).exists()
}

#[test]
fn without_an_option_both_projections_go_to_docs() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["render"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), "docs/spec.md\ndocs/spec.json\n");
    let markdown = fs::read_to_string(scratch.0.join("docs/spec.md")).unwrap();
    assert!(markdown.starts_with("# Reading List\n\n> loftspec/1 · state root `"));
    let export: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("docs/spec.json")).unwrap()).unwrap();
    assert_eq!(export["format"], "loftspec/1");
}

#[test]
fn each_option_writes_only_its_projection() {
    let scratch = Scratch::with_reading_list();
    assert_eq!(
        scratch
            .loft(&["render", "--markdown", "out/a.md"])
            .status
            .code(),
        Some(0)
    );
    assert!(exists(&scratch, "out/a.md") && !exists(&scratch, "docs"));
    assert_eq!(
        scratch
            .loft(&["render", "--json", "out/a.json"])
            .status
            .code(),
        Some(0)
    );
    assert!(exists(&scratch, "out/a.json") && !exists(&scratch, "docs"));
}

#[test]
fn an_unchanged_projection_is_not_rewritten() {
    let scratch = Scratch::with_reading_list();
    scratch.loft(&["render"]);
    let output = scratch.loft(&["render", "--format", "json"]);
    let document = json_of(&output);
    assert_eq!(document["outcome"], "rendered");
    assert_eq!(document["written"], Value::Array(Vec::new()));
    assert_eq!(document["unchanged"].as_array().unwrap().len(), 2);
}

#[test]
fn a_refused_render_is_an_error_in_json() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["render", "--markdown", "spec/out.md", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document = json_of(&output);
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["errors"][0]["kind"], "refused");
}

#[test]
fn findings_and_validation_errors_do_not_prevent_a_render() {
    let scratch = Scratch::with_reading_list();
    let product = "products/reading-list/product.yaml";
    scratch.edit(product, "  - portable\n", "  - portable\n  - portable\n");
    assert_eq!(scratch.loft(&["check"]).status.code(), Some(1));
    assert_eq!(scratch.loft(&["render"]).status.code(), Some(0));
    assert!(exists(&scratch, "docs/spec.md"));
}

#[test]
fn a_read_error_writes_no_projection() {
    let scratch = Scratch::with_reading_list();
    let product = "products/reading-list/product.yaml";
    scratch.edit(product, "\nslug:", " # a note\nslug:");
    let output = scratch.loft(&["render"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with(&format!("spec/{product}:1:")));
    assert!(!exists(&scratch, "docs"));
}

#[test]
fn an_output_under_spec_is_refused_and_nothing_is_written() {
    let scratch = Scratch::with_reading_list();
    let before = tree(&scratch);
    for path in ["spec/out.md", "docs/../spec/out.md", "./spec/deep/out.md"] {
        let output = scratch.loft(&["render", "--markdown", path, "--json", "docs/a.json"]);
        assert_eq!(output.status.code(), Some(1), "{path}");
        assert!(stderr(&output).starts_with(path), "{}", stderr(&output));
    }
    assert_eq!(tree(&scratch), before);
    assert!(!exists(&scratch, "docs"));
}

#[test]
fn a_render_changes_nothing_under_spec_and_writes_only_its_paths() {
    let scratch = Scratch::with_reading_list();
    let before = tree(&scratch);
    scratch.loft(&["render", "--markdown", "a.md"]);
    assert_eq!(tree(&scratch), before);
    let mut names: Vec<_> = fs::read_dir(&scratch.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["a.md", "spec"]);
}

#[test]
fn projections_change_no_root_and_raise_no_finding() {
    let scratch = Scratch::with_reading_list();
    let report = |scratch: &Scratch| stdout(&scratch.loft(&["check", "--format", "json"]));
    let before = report(&scratch);
    scratch.loft(&["render"]);
    assert_eq!(report(&scratch), before, "a present projection");
    fs::write(scratch.0.join("docs/spec.md"), "# Edited\n").unwrap();
    assert_eq!(report(&scratch), before, "an edited projection");
    fs::remove_dir_all(scratch.0.join("docs")).unwrap();
    assert_eq!(report(&scratch), before, "a deleted projection");
}

/// What `command` printed and the two projections it wrote, rendering the scratch tree into an
/// empty `docs/`.
fn rendered(scratch: &Scratch, command: &mut Command) -> [Vec<u8>; 3] {
    let _ = fs::remove_dir_all(scratch.0.join("docs"));
    let output = command.arg("render").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let written = |path: &str| fs::read(scratch.0.join(path)).unwrap();
    [
        output.stdout,
        written("docs/spec.md"),
        written("docs/spec.json"),
    ]
}

#[test]
fn the_projections_are_the_same_bytes_under_another_clock_locale_and_environment() {
    let scratch = Scratch::with_reading_list();
    let first = rendered(&scratch, &mut scratch.command(None, &Clock::fixed(EPOCH)));
    let mut other = scratch.command(None, &Clock::system());
    other
        .env("TZ", "Pacific/Kiritimati")
        .env("LANG", "tr_TR.UTF-8")
        .env("LC_ALL", "tr_TR.UTF-8")
        .env("LC_TIME", "ja_JP.UTF-8");
    assert_eq!(first, rendered(&scratch, &mut other));
}

#[test]
fn a_render_that_writes_its_projections_or_finds_them_current_is_rendered_in_json() {
    let scratch = Scratch::with_reading_list();
    let first = json_of(&scratch.loft(&["render", "--format", "json"]));
    assert_eq!(first["outcome"], "rendered");
    assert_eq!(first["written"].as_array().unwrap().len(), 2, "{first}");
    fs::remove_file(scratch.0.join("docs/spec.json")).unwrap();
    let mixed = json_of(&scratch.loft(&["render", "--format", "json"]));
    assert_eq!(mixed["outcome"], "rendered");
    assert_eq!(mixed["written"], serde_json::json!(["docs/spec.json"]));
    assert_eq!(mixed["unchanged"], serde_json::json!(["docs/spec.md"]));
}

#[test]
fn a_projection_that_cannot_be_written_exits_1_with_the_message_on_standard_error() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.0.join("out"), "a file, not a directory\n").unwrap();
    let output = scratch.loft(&["render", "--markdown", "out/spec.md"]);
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    let message = stderr(&output);
    assert!(message.contains("out: "), "{message}");
    assert_eq!(stdout(&output), "");
}

//! `loft merge-driver <base> <ours> <theirs> <path>` as git runs it (D-11 F-43): the merged file
//! written to `<ours>`, the exit code saying whether a conflict was written, and messages that
//! name `<path>` rather than git's temporary files.

mod common;
#[path = "common/schema.rs"]
mod schema;

use std::fs;
use std::process::Output;

use common::{Scratch, stderr, stdout};
use serde_json::{Value, json};

const FEATURE: &str = "products/reading-list/sharing/share-a-list.yaml";

const SHALL: &str = "shall: \"publish a read-only link.\"";

/// The three temporary files git hands the driver, beside `spec/` as git writes them.
const INPUTS: [&str; 3] = [".merge_file_base", ".merge_file_ours", ".merge_file_theirs"];

/// A scratch tree, and the Feature file's text as base.
fn setup() -> (Scratch, String) {
    let scratch = Scratch::with_reading_list();
    let base = fs::read_to_string(scratch.file(FEATURE)).unwrap();
    (scratch, base)
}

fn edited(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from:?}");
    text.replacen(from, to, 1)
}

/// Writes the three inputs and runs the driver for `path`.
fn driver(scratch: &Scratch, inputs: [&str; 3], path: &str) -> Output {
    driver_with(scratch, inputs, path, &[])
}

/// Writes the three inputs and runs the driver for `path`, followed by `args`.
fn driver_with(
    scratch: &Scratch,
    [base, ours, theirs]: [&str; 3],
    path: &str,
    args: &[&str],
) -> Output {
    for (name, text) in INPUTS.iter().zip([base, ours, theirs]) {
        fs::write(scratch.0.join(name), text).unwrap();
    }
    let [base, ours, theirs] = INPUTS;
    let mut all = vec!["merge-driver", base, ours, theirs, path];
    all.extend(args);
    scratch.loft(&all)
}

/// The one JSON document a run printed, held to the `loft merge-driver` schema.
fn json_of(output: &Output) -> Value {
    let document: Value = serde_json::from_str(&stdout(output)).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/merge-driver.json")).unwrap();
    assert_eq!(schema::violations(&document, &schema), Vec::<String>::new());
    document
}

const JSON: [&str; 2] = ["--format", "json"];

fn ours(scratch: &Scratch) -> String {
    fs::read_to_string(scratch.0.join(INPUTS[1])).unwrap()
}

fn spec_path() -> String {
    format!("spec/{FEATURE}")
}

#[test]
fn a_clean_merge_is_written_to_ours_and_exits_0() {
    let (scratch, base) = setup();
    let ours_text = edited(&base, SHALL, "shall: \"publish a link.\"");
    let theirs_text = edited(&base, "refuse edits made", "refuse every edit made");
    let output = driver(&scratch, [&base, &ours_text, &theirs_text], &spec_path());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "");
    let merged = ours(&scratch);
    assert!(merged.contains("shall: \"publish a link.\""), "{merged}");
    assert!(merged.contains("refuse every edit made"), "{merged}");
}

#[test]
fn a_written_conflict_exits_1_naming_the_path() {
    let (scratch, base) = setup();
    let ours_text = edited(&base, SHALL, "shall: \"publish a link.\"");
    let theirs_text = edited(&base, SHALL, "shall: \"publish a private link.\"");
    let output = driver(&scratch, [&base, &ours_text, &theirs_text], &spec_path());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        format!("{}: 1 merge conflict written\n", spec_path())
    );
    let merged = ours(&scratch);
    assert!(
        merged.contains("      - field: statement.shall\n"),
        "{merged}"
    );
    assert!(!stderr(&output).contains(".merge_file"));
}

#[test]
fn an_unreadable_input_is_named_with_where_and_ours_is_unchanged() {
    let (scratch, base) = setup();
    scratch.edit(FEATURE, "\nslug:", " # a note\nslug:");
    let theirs_text = fs::read_to_string(scratch.file(FEATURE)).unwrap();
    let output = driver(&scratch, [&base, &base, &theirs_text], &spec_path());
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.starts_with(&format!("{}:1:", spec_path())) && message.contains(": theirs: "),
        "{message}"
    );
    assert!(!message.contains(".merge_file"), "{message}");
    assert_eq!(ours(&scratch), base);
}

#[test]
fn a_path_below_the_top_level_is_merged_from_its_spec_directory_and_named_as_given() {
    let (scratch, base) = setup();
    let ours_text = edited(&base, SHALL, "shall: \"publish a link.\"");
    let theirs_text = edited(&base, SHALL, "shall: \"publish a private link.\"");
    let path = format!("sub/{}", spec_path());
    let output = driver(&scratch, [&base, &ours_text, &theirs_text], &path);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        format!("{path}: 1 merge conflict written\n")
    );
}

#[test]
fn a_path_with_no_spec_directory_exits_1_naming_it() {
    let (scratch, base) = setup();
    let output = driver(&scratch, [&base, &base, &base], "docs/notes.yaml");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "docs/notes.yaml: not under a `spec` directory\n"
    );
    assert_eq!(ours(&scratch), base);
}

#[test]
fn the_lint_configuration_changed_apart_exits_1_naming_the_path_and_ours_is_unchanged() {
    let scratch = Scratch::with_reading_list();
    let rule = |name: &str| format!("rules:\n  - rule: {name}\n    enabled: false\n");
    let (base, ours_text, theirs_text) = (rule("SH-002"), rule("SH-003"), rule("LQ-007"));
    let output = driver(
        &scratch,
        [&base, &ours_text, &theirs_text],
        "spec/lint.yaml",
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "spec/lint.yaml: both sides changed the lint configuration to different contents\n"
    );
    assert_eq!(ours(&scratch), ours_text);
}

#[test]
fn an_empty_base_is_an_absent_one_and_a_field_the_sides_disagree_on_keeps_ours() {
    let (scratch, base) = setup();
    let theirs_text = edited(
        &base,
        "while: \"a list is shared\"",
        "while: \"a list is public\"",
    );
    let output = driver(&scratch, ["", &base, &theirs_text], &spec_path());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let merged = ours(&scratch);
    assert!(merged.contains("while: \"a list is shared\""), "{merged}");
    assert!(
        merged.contains("      - field: statement.while\n        ours: \"a list is shared\"\n        theirs: \"a list is public\"\n"),
        "{merged}"
    );
}

#[test]
fn a_label_two_publishes_gave_apart_is_named_with_the_command_that_settles_it() {
    let scratch = Scratch::with_reading_list();
    let base = fs::read_to_string(scratch.file("labels.yaml")).unwrap();
    let binding =
        |n: u32| format!("  - key: 01a13099-7453-76e1-9b96-9210000000a{n}\n    label: RQ-90\n");
    let ours_text = format!("{base}{}", binding(1));
    let theirs_text = format!("{base}{}", binding(2));
    let output = driver(
        &scratch,
        [&base, &ours_text, &theirs_text],
        "spec/labels.yaml",
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "spec/labels.yaml: 1 merge conflict written\n\
         spec/labels.yaml: label RQ-90 was given apart by two publishes; run `loft publish` on \
         the publish branch to settle it\n"
    );
    assert!(ours(&scratch).contains("  - field: bindings\n"));
}

#[test]
fn the_json_document_of_a_merge_names_the_path_and_the_conflicts_written() {
    let (scratch, base) = setup();
    let ours_text = edited(&base, SHALL, "shall: \"publish a link.\"");
    let theirs_text = edited(&base, SHALL, "shall: \"publish a private link.\"");
    let conflicted = driver_with(
        &scratch,
        [&base, &ours_text, &theirs_text],
        &spec_path(),
        &JSON,
    );
    assert_eq!(conflicted.status.code(), Some(1));
    let document = json_of(&conflicted);
    assert_eq!(document["path"], spec_path());
    assert_eq!(
        (document["conflicts"].clone(), document["disputed"].clone()),
        (json!(1), json!([]))
    );
    let clean = driver_with(&scratch, [&base, &ours_text, &base], &spec_path(), &JSON);
    assert_eq!(clean.status.code(), Some(0), "{}", stderr(&clean));
    assert_eq!(json_of(&clean)["conflicts"], 0);
}

#[test]
fn the_json_document_of_a_merge_names_each_disputed_label() {
    let scratch = Scratch::with_reading_list();
    let base = fs::read_to_string(scratch.file("labels.yaml")).unwrap();
    let binding =
        |n: u32| format!("  - key: 01a13099-7453-76e1-9b96-9210000000a{n}\n    label: RQ-90\n");
    let ours_text = format!("{base}{}", binding(1));
    let theirs_text = format!("{base}{}", binding(2));
    let output = driver_with(
        &scratch,
        [&base, &ours_text, &theirs_text],
        "spec/labels.yaml",
        &JSON,
    );
    assert_eq!(json_of(&output)["disputed"], json!(["RQ-90"]));
}

#[test]
fn a_refused_merge_is_listed_among_the_json_document_s_errors() {
    let (scratch, base) = setup();
    let output = driver_with(
        &scratch,
        [&base, &base, &base],
        "elsewhere/file.yaml",
        &JSON,
    );
    assert_eq!(output.status.code(), Some(1));
    let error = &json_of(&output)["errors"][0];
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("elsewhere/file.yaml"),
        "{error}"
    );
}

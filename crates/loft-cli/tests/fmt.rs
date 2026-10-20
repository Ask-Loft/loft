//! `loft fmt` as a process (F-39, F-44, F-47): exit codes, the paths it prints, and what it
//! writes.

use std::fs;
use std::path::Path;
use std::process::Output;

use crate::common::{Scratch, stderr, stdout};
use crate::schema;
use crate::snapshot::snapshot;
use serde_json::{Value, json};

const PRODUCT: &str = "products/reading-list/product.yaml";

/// The one JSON document a run printed, held to the `loft fmt` schema.
fn json_of(output: &Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/fmt.json"))
}

/// The first file under `spec/decisions/`, as its path under `spec/`.
fn a_decision(root: &Path) -> String {
    let mut names: Vec<_> = fs::read_dir(root.join("spec/decisions"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    format!("decisions/{}", names[0])
}

#[test]
fn fmt_writes_the_key_of_a_reference_written_by_name_which_check_refuses() {
    let scratch = Scratch::with_reading_list();
    let product = fs::read_to_string(scratch.file(PRODUCT)).unwrap();
    let lines: Vec<&str> = product.lines().collect();
    let group = lines
        .iter()
        .position(|line| *line == "featureGroups:")
        .unwrap();
    let key = lines[group + 1].strip_prefix("  - key: ").unwrap();
    let name = lines[group + 1..]
        .iter()
        .find_map(|line| line.strip_prefix("    name: "))
        .unwrap();
    let named = product.replacen(
        "\nbody: ",
        &format!("\nmentions:\n  - name: {name}\nbody: "),
        1,
    );
    fs::write(scratch.file(PRODUCT), named).unwrap();
    let check = scratch.loft(&["check"]);
    assert_eq!(check.status.code(), Some(1));
    let report = stdout(&check) + &stderr(&check);
    assert!(report.contains("loft fmt"), "{report}");
    let write = scratch.loft(&["fmt"]);
    assert_eq!(write.status.code(), Some(0), "{}", stderr(&write));
    let formatted = fs::read_to_string(scratch.file(PRODUCT)).unwrap();
    assert!(
        formatted.contains(&format!("mentions:\n  - {key}\n")),
        "{formatted}"
    );
}

#[test]
fn a_canonical_tree_checks_clean_and_formats_to_nothing() {
    let scratch = Scratch::with_reading_list();
    let check = scratch.loft(&["fmt", "--check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
    assert_eq!(stdout(&check), "");
    let write = scratch.loft(&["fmt"]);
    assert_eq!(write.status.code(), Some(0), "{}", stderr(&write));
    assert_eq!(stdout(&write), "");
}

#[test]
fn check_names_a_drifting_file_and_writes_nothing() {
    let scratch = Scratch::with_reading_list();
    let path = a_decision(&scratch.0);
    scratch.edit(&path, "title: \"", "title:   \"");
    let before = fs::read(scratch.file(&path)).unwrap();
    let check = scratch.loft(&["fmt", "--check"]);
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(stdout(&check), format!("spec/{path}\n"));
    assert_eq!(fs::read(scratch.file(&path)).unwrap(), before);
}

#[test]
fn fmt_rewrites_a_drifting_file_and_then_checks_clean() {
    let scratch = Scratch::with_reading_list();
    let path = a_decision(&scratch.0);
    scratch.edit(&path, "title: \"", "title:   \"");
    let write = scratch.loft(&["fmt"]);
    assert_eq!(write.status.code(), Some(0), "{}", stderr(&write));
    assert_eq!(stdout(&write), format!("spec/{path}\n"));
    assert_eq!(scratch.loft(&["fmt", "--check"]).status.code(), Some(0));
    let leftovers = fs::read_dir(&scratch.0).unwrap().count();
    assert_eq!(leftovers, 1, "only spec/ remains beside the tree");
}

#[test]
fn a_comment_is_an_error_at_its_position_and_nothing_is_written() {
    let scratch = Scratch::with_reading_list();
    let path = a_decision(&scratch.0);
    scratch.edit(&path, "\nslug:", " # a note\nslug:");
    let before = fs::read(scratch.file(&path)).unwrap();
    let output = scratch.loft(&["fmt"]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.starts_with(&format!("spec/{path}:1:")), "{message}");
    assert_eq!(fs::read(scratch.file(&path)).unwrap(), before);
}

#[test]
fn a_directory_without_a_tree_is_an_error() {
    let scratch = Scratch::with_reading_list();
    fs::remove_dir_all(scratch.0.join("spec")).unwrap();
    let output = scratch.loft(&["fmt", "--check"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("there is no `spec/` directory"));
}

#[test]
fn a_usage_error_exits_2() {
    let scratch = Scratch::with_reading_list();
    assert_eq!(scratch.loft(&["fmt", "--chekc"]).status.code(), Some(2));
    assert_eq!(scratch.loft(&["frobnicate"]).status.code(), Some(2));
}

#[test]
fn the_json_document_lists_the_files_rewritten_and_the_keys_minted() {
    let scratch = Scratch::with_reading_list();
    let path = a_decision(&scratch.0);
    scratch.edit(&path, "title: \"", "title:   \"");
    scratch.edit(
        PRODUCT,
        "      - key: 00000000-0000-4000-8000-000000000032\n",
        "      - name: \"A new criterion.\"\n      - key: 00000000-0000-4000-8000-000000000032\n",
    );
    let mut changed = vec![format!("spec/{path}"), format!("spec/{PRODUCT}")];
    changed.sort();
    let document = |outcome: &str, changed: &[String], minted: usize, written: bool| {
        json!({
            "schema": "loft-fmt/2",
            "outcome": outcome,
            "changed": changed,
            "minted": minted,
            "written": written,
            "errors": [],
        })
    };
    let check = scratch.loft(&["fmt", "--check", "--format", "json"]);
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(json_of(&check), document("drift", &changed, 1, false));
    let write = scratch.loft(&["fmt", "--format", "json"]);
    assert_eq!(write.status.code(), Some(0), "{}", stderr(&write));
    assert_eq!(json_of(&write), document("formatted", &changed, 1, true));
    let again = scratch.loft(&["fmt", "--format", "json"]);
    assert_eq!(json_of(&again), document("canonical", &[], 0, false));
}

#[test]
fn a_read_error_is_listed_in_the_json_document_with_its_file_line_and_column() {
    let scratch = Scratch::with_reading_list();
    let path = a_decision(&scratch.0);
    scratch.edit(&path, "\nslug:", " # a note\nslug:");
    let output = scratch.loft(&["fmt", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document = json_of(&output);
    assert_eq!(document["outcome"], "error");
    let error = &document["errors"][0];
    assert_eq!(error["kind"], "read");
    assert_eq!(error["path"], format!("spec/{path}"));
    assert_eq!(error["line"], 1);
    assert!(error["column"].is_u64());
}

#[test]
fn fmt_marks_departed_the_binding_of_a_deleted_node_so_that_check_passes() {
    let scratch = Scratch::with_reading_list();
    let deleted = "00000000-0000-4000-8000-00000000002a";
    let labels = || fs::read_to_string(scratch.file("labels.yaml")).unwrap();
    assert!(labels().contains(deleted));
    scratch.edit(
        "products/reading-list/shelf/add-a-book.yaml",
        &format!(
            "      - key: {deleted}\n        name: \"A book added without a title is refused with an error naming the title.\"\n"
        ),
        "",
    );
    assert_eq!(scratch.loft(&["check"]).status.code(), Some(1));
    let output = scratch.loft(&["fmt"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("spec/labels.yaml"),
        "{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains(&format!("({deleted}) names no node or decision")),
        "{}",
        stderr(&output)
    );
    let departed = format!("  - key: {deleted}\n    label: AC-2\n    departed: true\n");
    assert!(labels().contains(&departed), "{}", labels());
    let check = scratch.loft(&["check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stdout(&check));
}

#[test]
fn a_reference_by_name_that_matches_nothing_or_two_elements_changes_no_file() {
    for (name, renamed) in [("Nothing", None), ("Shelf", Some("Sharing"))] {
        let scratch = Scratch::with_reading_list();
        let path = a_decision(&scratch.0);
        scratch.edit(&path, "title: \"", "title:   \"");
        if let Some(other) = renamed {
            scratch.edit(
                PRODUCT,
                &format!("name: \"{other}\""),
                &format!("name: \"{name}\""),
            );
        }
        scratch.edit(
            PRODUCT,
            "\nbody: ",
            &format!("\nmentions:\n  - name: \"{name}\"\nbody: "),
        );
        let before = snapshot(&scratch.0);
        let output = scratch.loft(&["fmt"]);
        assert_eq!(output.status.code(), Some(1), "{name}: {}", stdout(&output));
        assert!(stderr(&output).contains(name), "{}", stderr(&output));
        assert_eq!(snapshot(&scratch.0), before, "{name}: a file changed");
    }
}

#[test]
fn each_binding_newly_marked_departed_is_reported_on_standard_error_with_its_label_and_key() {
    let scratch = Scratch::with_reading_list();
    let deleted = "00000000-0000-4000-8000-00000000002a";
    scratch.edit(
        "products/reading-list/shelf/add-a-book.yaml",
        &format!(
            "      - key: {deleted}\n        name: \"A book added without a title is refused with an error naming the title.\"\n"
        ),
        "",
    );
    let output = scratch.loft(&["fmt"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stderr(&output).contains(&format!("the binding of AC-2 ({deleted})")),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("marks it departed"),
        "{}",
        stderr(&output)
    );
    let again = scratch.loft(&["fmt"]);
    assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
    assert!(
        !stderr(&again).contains("the binding of"),
        "{}",
        stderr(&again)
    );
}

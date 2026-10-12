//! `loft import v1` as a process (D-11 F-37): the tree and sidecar it writes, its findings and
//! counters warning, and the refusals that write nothing.

#[path = "common/schema.rs"]
mod schema;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, process};

use loftspec::codec;
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;
use serde_json::Value;

const READING_LIST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/v1/reading-list/loft-authored-state.json"
);

/// An empty directory, removed when the test ends.
struct Empty(PathBuf);

impl Empty {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("loft-cli-import-{}-{number}", process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn loft(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_loft"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }

    fn import(&self, extra: &[&str]) -> Output {
        let mut args = vec!["import", "v1", READING_LIST];
        args.extend_from_slice(extra);
        self.loft(&args)
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }

    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Empty {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// Every file under `<root>/spec/`, by its path under `spec/`.
fn tree(root: &Path) -> codec::Files {
    loftspec_store::fs::read_tree(root).unwrap()
}

fn converted() -> loftspec::convert::v1::Conversion {
    convert(&Export::from_json(&fs::read_to_string(READING_LIST).unwrap()).unwrap()).unwrap()
}

#[test]
fn ac_506_ac_508_the_tree_is_the_converters_output_and_the_sidecar_lies_beside_it() {
    let dir = Empty::new();
    let output = dir.import(&[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let conversion = converted();
    assert_eq!(tree(&dir.0), codec::write(&conversion.document));
    assert_eq!(
        fs::read_to_string(dir.0.join("v1-sidecar.json")).unwrap(),
        conversion.sidecar.to_json()
    );
    assert_eq!(dir.loft(&["fmt", "--check"]).status.code(), Some(0));
}

#[test]
fn ac_509_the_findings_print_as_a_lint_report() {
    let dir = Empty::new();
    let output = dir.import(&[]);
    assert_eq!(
        stdout(&output),
        "spec/products/reading-list/shelf/remove-a-book.yaml:16:10: info V2-002: The statement \
         is stored as text, without slots.\nspec/products/reading-list/product.yaml:18:10: info \
         V2-001: The body holds a heading; use a Section. (body line 3)\n"
    );
}

#[test]
fn ac_515_ac_516_without_counters_each_is_one_above_the_highest_and_a_warning_says_so() {
    let dir = Empty::new();
    let output = dir.import(&[]);
    let labels = fs::read_to_string(dir.0.join("spec/labels.yaml")).unwrap();
    assert!(labels.contains("  RQ: 7\n"), "{labels}");
    let warning = stderr(&output);
    assert!(
        warning.contains("the counters for P, FG, F, RQ, AC, CMP, CON, QA, ADR"),
        "{warning}"
    );
    assert!(
        warning.contains("the source may have allocated higher values"),
        "{warning}"
    );
}

#[test]
fn ac_510_ac_511_ac_512_a_counters_file_sets_the_prefixes_it_names() {
    let dir = Empty::new();
    dir.write("counters.yaml", "RQ: 40\nAC: 90\n");
    let output = dir.import(&["--counters", "counters.yaml"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let labels = fs::read_to_string(dir.0.join("spec/labels.yaml")).unwrap();
    assert!(
        labels.contains("  RQ: 40\n") && labels.contains("  AC: 90\n"),
        "{labels}"
    );
    assert!(
        labels.contains("  CMP: 5\n"),
        "an absent prefix is one above the highest"
    );
    assert!(!stderr(&output).contains("RQ, AC"), "{}", stderr(&output));
}

#[test]
fn ac_513_ac_514_a_counter_not_above_the_highest_label_is_refused_and_nothing_is_written() {
    let dir = Empty::new();
    dir.write("counters.yaml", "RQ: 6\n");
    let output = dir.import(&["--counters", "counters.yaml"]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(
        message.contains("`RQ`") && message.contains(" 6 ") && message.ends_with("6\n"),
        "{message}"
    );
    assert_eq!(dir.entries(), ["counters.yaml"]);
}

#[test]
fn ac_517_ac_518_a_spec_directory_that_is_not_empty_is_refused() {
    let dir = Empty::new();
    fs::create_dir(dir.0.join("spec")).unwrap();
    dir.write("spec/loftspec.yaml", "format: loftspec/1\n");
    let output = dir.import(&[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("spec"), "{}", stderr(&output));
    assert_eq!(dir.entries(), ["spec"]);
    assert_eq!(tree(&dir.0).len(), 1);
}

#[test]
fn a_spec_directory_holding_only_dot_entries_counts_as_empty() {
    let dir = Empty::new();
    fs::create_dir(dir.0.join("spec")).unwrap();
    dir.write("spec/.DS_Store", "");
    let output = dir.import(&[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(dir.0.join("spec/loftspec.yaml").is_file());
}

#[test]
fn an_empty_spec_directory_is_accepted() {
    let dir = Empty::new();
    fs::create_dir(dir.0.join("spec")).unwrap();
    assert_eq!(dir.import(&[]).status.code(), Some(0));
}

#[test]
fn ac_519_ac_521_a_missing_export_is_reported_by_its_path() {
    let dir = Empty::new();
    let output = dir.loft(&["import", "v1", "absent.json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("absent.json: "),
        "{}",
        stderr(&output)
    );
    assert!(dir.entries().is_empty());
}

#[test]
fn ac_520_a_file_that_is_not_an_export_names_its_path_and_the_first_missing_field() {
    let dir = Empty::new();
    dir.write("other.json", "{}");
    let output = dir.loft(&["import", "v1", "other.json"]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.starts_with("other.json: "), "{message}");
    assert!(
        message.contains("missing field `authoredStateId`"),
        "{message}"
    );
    assert_eq!(dir.entries(), ["other.json"]);
}

#[test]
fn ac_668_ac_671_a_missing_counters_file_is_reported_by_its_path() {
    let dir = Empty::new();
    let output = dir.import(&["--counters", "absent.yaml"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("absent.yaml: "),
        "{}",
        stderr(&output)
    );
    assert!(dir.entries().is_empty());
}

#[test]
fn ac_669_a_prefix_outside_the_formats_is_reported_by_name() {
    let dir = Empty::new();
    dir.write("counters.yaml", "XX: 3\n");
    let output = dir.import(&["--counters", "counters.yaml"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("`XX`"), "{}", stderr(&output));
    assert_eq!(dir.entries(), ["counters.yaml"]);
}

#[test]
fn ac_670_a_value_that_is_not_a_positive_integer_is_reported_with_its_prefix() {
    let dir = Empty::new();
    for value in ["0", "-3", "seven", "1.5"] {
        dir.write("counters.yaml", &format!("RQ: {value}\n"));
        let output = dir.import(&["--counters", "counters.yaml"]);
        assert_eq!(output.status.code(), Some(1), "{value}");
        let message = stderr(&output);
        assert!(
            message.contains("`RQ`") && message.contains(value),
            "{message}"
        );
    }
    assert_eq!(dir.entries(), ["counters.yaml"]);
}

#[test]
fn ac_582_json_output_matches_the_schema() {
    let schema: Value = serde_json::from_str(include_str!("../../../schemas/import.json")).unwrap();
    let dir = Empty::new();
    let output = dir.import(&["--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let document: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(schema::violations(&document, &schema), Vec::<String>::new());
    assert_eq!(document["findings"].as_array().unwrap().len(), 2);
    assert!(
        document["written"]
            .as_array()
            .unwrap()
            .contains(&"v1-sidecar.json".into())
    );
    let refused = dir.import(&["--format", "json"]);
    assert_eq!(refused.status.code(), Some(1));
    let document: Value = serde_json::from_str(&stdout(&refused)).unwrap();
    assert_eq!(schema::violations(&document, &schema), Vec::<String>::new());
    assert_eq!(document["errors"].as_array().unwrap().len(), 1);
}

//! Reading the tree: the rules every command tagged `reads-tree` follows on a reader error, run
//! against each command whose Feature in this repository's own `spec/` lists the tag; and the
//! converse, that every command that reports a read error lists it.

use std::fs;
use std::path::{Path, PathBuf};

use loftspec_store::FileStore;
use serde_json::{Value, json};

use crate::commands::COMMANDS;
use crate::common::{Scratch, stderr, stdout};
use crate::conformance::{tagged, tagged_commands};
use crate::schema::conforming;
use crate::snapshot::snapshot;

const PRODUCT: &str = "products/reading-list/product.yaml";

const TAG: &str = "reads-tree";

fn schemas() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas")
}

/// A scratch tree whose product file holds a comment, which the reader refuses at line 1.
pub fn broken() -> Scratch {
    let scratch = Scratch::with_reading_list();
    scratch.edit(PRODUCT, "\nslug:", " # a note\nslug:");
    scratch
}

/// What the command printed, standard output then standard error.
fn output_text(output: &std::process::Output) -> String {
    stdout(output) + &stderr(output)
}

#[test]
fn each_tagged_command_reports_the_error_changes_no_file_and_exits_1() {
    for (slug, args, _, _) in tagged_commands(TAG) {
        let scratch = broken();
        let before = snapshot(&scratch.0);
        let output = scratch.loft(args);
        assert_eq!(output.status.code(), Some(1), "{slug}: the exit code");
        let text = output_text(&output);
        assert!(
            text.lines()
                .any(|line| line.starts_with(&format!("spec/{PRODUCT}:1:"))),
            "{slug}: the error at its file, line and column\n{text}"
        );
        assert!(snapshot(&scratch.0) == before, "{slug}: a file changed");
    }
}

#[test]
fn each_command_that_reports_a_read_error_lists_reads_tree() {
    let tagged = tagged(TAG);
    for (slug, args, _, _) in COMMANDS {
        let output = broken().loft(args);
        let reports = output_text(&output)
            .lines()
            .any(|line| line.starts_with(&format!("spec/{PRODUCT}:1:")));
        assert_eq!(
            reports,
            tagged.iter().any(|tag| tag == slug),
            "{slug}: reports a read error exactly when its Feature lists `reads-tree`"
        );
    }
}

/// The kind name the reader gives the problem of each read error of the tree in `scratch`.
fn problems(scratch: &Scratch) -> Vec<String> {
    let files = FileStore::open(&scratch.0).unwrap().read_files().unwrap();
    let errors = loftspec::codec::read(&files).expect_err("the tree is refused");
    errors
        .iter()
        .map(|error| error.problem.kind().to_owned())
        .collect()
}

#[test]
fn each_tagged_command_names_the_readers_problem_in_each_read_error_of_its_json() {
    let expected = problems(&broken());
    assert_eq!(expected, ["syntax"]);
    for (slug, args, _, schema) in tagged_commands(TAG) {
        let output = broken().loft(&[args, &["--format", "json"]].concat());
        let schema = fs::read_to_string(schemas().join(format!("{schema}.json"))).unwrap();
        let document = conforming(&output.stdout, &schema);
        let errors = document["errors"].as_array().unwrap();
        let read: Vec<&Value> = errors
            .iter()
            .filter(|error| error["kind"] == "read")
            .collect();
        assert_eq!(read.len(), expected.len(), "{slug}: {document}");
        for (error, problem) in read.iter().zip(&expected) {
            assert_eq!(error["problem"], json!(problem), "{slug}: {error}");
        }
        for error in errors.iter().filter(|error| error["kind"] != "read") {
            assert_eq!(error.get("problem"), None, "{slug}: {error}");
        }
    }
}

#[test]
fn an_error_whose_kind_is_not_read_names_no_problem() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.file("lint.yaml"), "rules:\n  - rule: XX-001\n").unwrap();
    for args in [
        &["check", "--format", "json"][..],
        &["terms", "--format", "json"],
    ] {
        let document: Value = serde_json::from_slice(&scratch.loft(args).stdout).unwrap();
        let errors = document["errors"].as_array().unwrap();
        assert_eq!(errors[0]["kind"], "lint-config", "{args:?}: {document}");
        for error in errors {
            assert_eq!(error.get("problem"), None, "{args:?}: {error}");
        }
    }
}

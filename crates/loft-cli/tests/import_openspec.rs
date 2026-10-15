//! `loft import openspec` as a process: the tree and sidecar it writes from an `OpenSpec`
//! project, the files it gives the converter, its findings and `V2-007` report, and the
//! refusals that write nothing.

use std::path::{Path, PathBuf};
use std::{fs, os::unix::fs::symlink};

use crate::common::{Scratch, stderr, stdout, tree};
use crate::schema;
use crate::snapshot::snapshot;
use loftspec::codec;
use loftspec::convert::openspec::{Project, convert};
use loftspec::document::Document;
use loftspec_store::{DocumentStore, FileStore};
use serde_json::Value;

const SCHEMA: &str = include_str!("../../../schemas/import-openspec.json");

/// The `OpenSpec` directory of a fixture project under `testdata/openspec/`.
fn fixture(project: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/openspec")
        .join(project)
        .join("openspec")
}

/// Copies the directory `from` to `to`, recursively.
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A scratch directory holding a fixture project's `OpenSpec` directory as `openspec/`.
fn holding(project: &str) -> Scratch {
    let scratch = Scratch::empty();
    copy(&fixture(project), &scratch.0.join("openspec"));
    scratch
}

fn write(scratch: &Scratch, path: &str, bytes: &[u8]) {
    let target = scratch.0.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, bytes).unwrap();
}

fn document(root: &Path) -> Document {
    FileStore::open(root).unwrap().load().unwrap()
}

/// Asserts that `loft import openspec` with `args`, run in `scratch`, refuses, exits 1 with an
/// error that contains `named`, and writes nothing.
fn refuses(scratch: &Scratch, args: &[&str], named: &str) {
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&[&["import", "openspec"], args].concat());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains(named), "{}", stderr(&output));
    assert_eq!(snapshot(&scratch.0), before);
}

/// The faithful fixture, as the files its `OpenSpec` directory holds.
fn faithful_project() -> Project {
    let dir = fixture("faithful");
    let read = |path: &str| fs::read_to_string(dir.join(path)).unwrap();
    Project {
        specs: vec![
            ("auth/spec.md".to_owned(), read("specs/auth/spec.md")),
            (
                "billing/invoices/spec.md".to_owned(),
                read("specs/billing/invoices/spec.md"),
            ),
        ],
        project_md: Some(read("project.md")),
        config_yaml: Some(read("config.yaml")),
        active_changes: vec!["add-refunds".to_owned()],
        archived_changes: 2,
    }
}

#[test]
fn the_tree_is_the_converters_output_for_its_document_key_and_the_sidecar_lies_beside_it() {
    let scratch = Scratch::empty();
    let dir = fixture("faithful");
    let output = scratch.loft(&["import", "openspec", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let key = document(&scratch.0).id;
    let conversion = convert(&faithful_project(), key, "faithful").unwrap();
    assert_eq!(tree(&scratch.0), codec::write(&conversion.document));
    assert_eq!(
        fs::read_to_string(scratch.0.join("openspec-sidecar.json")).unwrap(),
        conversion.sidecar.to_json()
    );
    assert_eq!(scratch.loft(&["fmt", "--check"]).status.code(), Some(0));
}

#[test]
fn without_a_directory_the_project_is_read_from_openspec_and_named_after_its_holder() {
    let scratch = holding("faithful");
    let output = scratch.loft(&["import", "openspec"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let holder = scratch.0.file_name().unwrap().to_str().unwrap().to_owned();
    assert_eq!(document(&scratch.0).products[0].name, holder);
}

#[test]
fn the_product_is_named_by_the_product_option() {
    let scratch = holding("faithful");
    let output = scratch.loft(&["import", "openspec", "--product", "Map Shop"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(document(&scratch.0).products[0].name, "Map Shop");
}

#[test]
fn each_import_mints_a_new_document_key() {
    let (first, second) = (holding("faithful"), holding("faithful"));
    for scratch in [&first, &second] {
        assert_eq!(scratch.loft(&["import", "openspec"]).status.code(), Some(0));
    }
    assert_ne!(document(&first.0).id, document(&second.0).id);
}

#[test]
fn an_import_allocates_no_label_and_writes_no_lock_or_trace() {
    let scratch = holding("faithful");
    assert_eq!(scratch.loft(&["import", "openspec"]).status.code(), Some(0));
    assert!(document(&scratch.0).bindings.is_empty());
    assert!(!scratch.0.join("loft.lock.json").exists());
    assert!(!scratch.0.join("loft.trace.yaml").exists());
}

#[test]
fn a_spec_reached_through_a_symbolic_link_is_not_given() {
    let scratch = holding("quiet");
    symlink(
        scratch.0.join("openspec/specs/notes"),
        scratch.0.join("openspec/specs/linked"),
    )
    .unwrap();
    fs::create_dir_all(scratch.0.join("openspec/specs/aliased")).unwrap();
    symlink(
        scratch.0.join("openspec/specs/notes/spec.md"),
        scratch.0.join("openspec/specs/aliased/spec.md"),
    )
    .unwrap();
    let output = scratch.loft(&["import", "openspec"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let sidecar = fs::read_to_string(scratch.0.join("openspec-sidecar.json")).unwrap();
    assert!(sidecar.contains("specs/notes/spec.md"), "{sidecar}");
    assert!(
        !sidecar.contains("linked") && !sidecar.contains("aliased"),
        "{sidecar}"
    );
}

#[test]
fn active_changes_are_the_directories_under_changes_and_archived_ones_are_counted_in_both_places() {
    let scratch = holding("quiet");
    for dir in [
        "changes/add-tags",
        "changes/archive/2026-01-01-add-notes",
        "archive/2025-12-01-add-maps",
        "archive/2025-12-02-add-shop",
    ] {
        fs::create_dir_all(scratch.0.join("openspec").join(dir)).unwrap();
    }
    write(&scratch, "openspec/changes/README.md", b"Changes.\n");
    write(&scratch, "openspec/changes/archive/.gitkeep", b"");
    let output = scratch.loft(&["import", "openspec"]);
    let text = stdout(&output);
    assert!(
        text.contains("info OS-011: active changes were not converted: 1 (add-tags)\n"),
        "{text}"
    );
    assert!(
        text.contains("info OS-012: archived changes were not converted: 3\n"),
        "{text}"
    );
}

#[test]
fn findings_print_one_per_line_with_their_source_file_and_line() {
    let scratch = holding("faithful");
    let output = scratch.loft(&["import", "openspec"]);
    assert_eq!(
        stdout(&output),
        "info OS-011: active changes were not converted: 1 (add-refunds)\n\
         info OS-012: archived changes were not converted: 2\n\
         openspec/config.yaml:2: info OS-013: the context of config.yaml was not converted\n\
         info OS-008: statements whose SHALL or MUST was written as shall: 3\n"
    );
}

/// The `V2-007` warnings `loft check` reports on the tree in `scratch`.
fn modal_warnings(scratch: &Scratch) -> usize {
    let output = scratch.loft(&["check", "--format", "json"]);
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    document["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == "V2-007" && finding["severity"] == "warning")
        .count()
}

#[test]
fn the_report_counts_the_modal_warnings_and_shows_the_entry_that_lowers_them() {
    let scratch = holding("faithful");
    let output = scratch.loft(&["import", "openspec"]);
    let message = stderr(&output);
    let warnings = modal_warnings(&scratch);
    assert_eq!(warnings, 4);
    assert!(
        message.contains(&format!("`loft check` reports {warnings} V2-007 warnings")),
        "{message}"
    );
    let entry = "rules:\n  - rule: V2-007\n    severity: info\n";
    assert!(message.contains(entry), "{message}");
    assert!(!scratch.0.join("spec/lint.yaml").exists());
    write(&scratch, "spec/lint.yaml", entry.as_bytes());
    assert_eq!(modal_warnings(&scratch), 0);
}

#[test]
fn a_tree_without_modal_warnings_gets_no_report() {
    let scratch = holding("quiet");
    let output = scratch.loft(&["import", "openspec"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(modal_warnings(&scratch), 0);
    assert!(!stderr(&output).contains("V2-007"), "{}", stderr(&output));
}

#[test]
fn a_spec_directory_that_is_not_empty_is_refused_and_nothing_is_written() {
    let scratch = holding("quiet");
    write(&scratch, "spec/kept.yaml", b"kept\n");
    refuses(&scratch, &[], "./spec: `spec/` is not empty");
}

#[test]
fn a_hidden_entry_does_not_make_spec_not_empty() {
    let scratch = holding("quiet");
    write(&scratch, "spec/.keep", b"");
    let output = scratch.loft(&["import", "openspec"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

#[test]
fn a_directory_that_cannot_be_read_is_refused_by_its_path() {
    refuses(&Scratch::empty(), &["absent"], "absent");
}

#[test]
fn a_project_without_main_specs_is_refused_naming_its_active_changes() {
    refuses(&holding("delta-only"), &[], "add-notes");
}

#[test]
fn a_file_that_is_not_utf8_is_refused_by_its_path() {
    for file in ["openspec/specs/broken/spec.md", "openspec/project.md"] {
        let scratch = holding("quiet");
        write(&scratch, file, &[b'#', b' ', 0xff, b'\n']);
        refuses(&scratch, &[], file);
    }
}

#[test]
fn json_output_matches_the_schema() {
    let scratch = holding("faithful");
    let output = scratch.loft(&["import", "openspec", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = schema::conforming(&output.stdout, SCHEMA);
    assert_eq!(document["outcome"], "imported");
    assert_eq!(document["findings"].as_array().unwrap().len(), 4);
    assert!(
        document["written"]
            .as_array()
            .unwrap()
            .contains(&"openspec-sidecar.json".into())
    );
    assert_eq!(document["advice"][0]["warnings"], 4);
    let refused = scratch.loft(&["import", "openspec", "--format", "json"]);
    assert_eq!(refused.status.code(), Some(1));
    let document = schema::conforming(&refused.stdout, SCHEMA);
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["errors"].as_array().unwrap().len(), 1);
}

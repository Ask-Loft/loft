//! `loft fmt` as a process (D-11 F-39, F-44, F-47): exit codes, the paths it prints, and what it
//! writes.

mod common;

use std::fs;
use std::path::Path;

use common::{Scratch, stderr, stdout};

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

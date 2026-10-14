//! The limits `AGENTS.md` sets on the size of the code, held by guards: every Rust source file
//! under `crates/` holds at most 500 lines, no source suppresses Clippy's length gate, and
//! `clippy.toml` keeps its complexity thresholds. A file that crosses the ceiling is split, never
//! given a budget.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::workspace;

/// The most lines a Rust source file may hold.
const CEILING: usize = 500;

/// Every Rust source file beneath `dir`.
fn sources(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    found
}

/// Each file of `files`, by name and text, that holds more lines than the ceiling, with its
/// count.
fn over_ceiling(files: &[(String, String)]) -> Vec<(String, usize)> {
    files
        .iter()
        .map(|(name, text)| (name.clone(), text.lines().count()))
        .filter(|(_, lines)| *lines > CEILING)
        .collect()
}

/// Every Rust source file under `crates/`, by its path from the workspace and its text.
fn workspace_sources() -> Vec<(String, String)> {
    let workspace = workspace();
    sources(&workspace.join("crates"))
        .into_iter()
        .map(|path| {
            let name = path.strip_prefix(&workspace).unwrap().display().to_string();
            (name, fs::read_to_string(&path).unwrap())
        })
        .collect()
}

#[test]
fn every_rust_source_file_holds_at_most_the_ceiling_of_lines() {
    let files = workspace_sources();
    assert!(files.len() > 200, "the scan found {} files", files.len());
    assert_eq!(over_ceiling(&files), Vec::<(String, usize)>::new());
}

#[test]
fn a_file_over_the_ceiling_is_reported_with_its_count() {
    let at = "line\n".repeat(CEILING);
    let over = "line\n".repeat(CEILING + 1);
    let files = [("at.rs".to_owned(), at), ("over.rs".to_owned(), over)];
    assert_eq!(over_ceiling(&files), [("over.rs".to_owned(), CEILING + 1)]);
}

#[test]
fn no_rust_source_suppresses_the_length_gate() {
    // Written in two parts, so that this file does not name the lint it looks for.
    let lint = ["too_many_", "lines"].concat();
    let suppressing: Vec<String> = workspace_sources()
        .into_iter()
        .filter(|(_, text)| text.contains(&lint))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(suppressing, Vec::<String>::new());
}

#[test]
fn clippy_toml_keeps_its_complexity_thresholds() {
    let configuration = fs::read_to_string(workspace().join("clippy.toml")).unwrap();
    for setting in [
        "too-many-lines-threshold = 80",
        "excessive-nesting-threshold = 4",
        "too-many-arguments-threshold = 6",
        "type-complexity-threshold = 200",
    ] {
        assert!(
            configuration.lines().any(|line| line == setting),
            "clippy.toml must keep {setting}"
        );
    }
}

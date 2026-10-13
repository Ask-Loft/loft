//! Test names cite no label: a publish that meets another can withdraw a label and give its node
//! a new one, so a name like `ac_454_…` would go on naming a criterion that no longer holds it.
//! A guard keeps every test name free of a label.

use crate::common::workspace;
use std::fs;
use std::path::{Path, PathBuf};

/// The label prefixes, as they would appear in a name.
const PREFIXES: [&str; 9] = ["p", "fg", "f", "rq", "ac", "cmp", "con", "qa", "adr"];

/// The labels a test name holds anywhere: `ac_165_and_ac_166_…` holds AC-165 and AC-166.
fn cited(name: &str) -> Vec<String> {
    name.split('_')
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|pair| PREFIXES.contains(&pair[0]) && pair[1].bytes().all(|b| b.is_ascii_digit()))
        .map(|pair| format!("{}-{}", pair[0].to_uppercase(), pair[1]))
        .collect()
}

/// Every Rust source file under each crate's `src/` and `tests/`.
fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// The name of each `#[test]` function in `text`.
fn test_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut after_test = false;
    for line in text.lines().map(str::trim) {
        if line == "#[test]" {
            after_test = true;
        } else if after_test && let Some(rest) = line.strip_prefix("fn ") {
            names.push(rest.split('(').next().unwrap_or_default().to_owned());
            after_test = false;
        }
    }
    names
}

/// Each label a name in `names` holds, as `file: name: label`.
fn citations(names: &[(String, String)]) -> Vec<String> {
    names
        .iter()
        .flat_map(|(file, name)| {
            cited(name)
                .into_iter()
                .map(move |label| format!("{file}: {name}: {label}"))
        })
        .collect()
}

#[test]
fn no_test_name_cites_a_label() {
    let mut files = Vec::new();
    for entry in fs::read_dir(workspace().join("crates")).unwrap() {
        let krate = entry.unwrap().path();
        for part in ["src", "tests"] {
            if krate.join(part).is_dir() {
                sources(&krate.join(part), &mut files);
            }
        }
    }
    let names: Vec<(String, String)> = files
        .iter()
        .flat_map(|file| {
            let shown = file
                .strip_prefix(workspace())
                .unwrap()
                .display()
                .to_string();
            test_names(&fs::read_to_string(file).unwrap())
                .into_iter()
                .map(move |name| (shown.clone(), name))
        })
        .collect();
    assert!(names.len() > 500, "the scan found {} tests", names.len());
    assert_eq!(citations(&names), Vec::<String>::new());
}

#[test]
fn a_label_is_found_anywhere_in_a_name() {
    assert_eq!(cited("ac_454_every_node"), ["AC-454"]);
    assert_eq!(cited("ac_165_and_ac_166_both"), ["AC-165", "AC-166"]);
    assert_eq!(cited("a_kind_rq_74"), ["RQ-74"]);
    assert_eq!(cited("a_scenario_round_trips"), Vec::<String>::new());
    assert_eq!(cited("two_publishes_meet"), Vec::<String>::new());
}

#[test]
fn a_test_name_citing_a_label_is_reported() {
    let names = [("t.rs".to_owned(), "ac_1_x".to_owned())];
    assert_eq!(citations(&names), ["t.rs: ac_1_x: AC-1"]);
}

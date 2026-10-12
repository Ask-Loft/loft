//! Tests cite the labels of the criteria they prove (`ac_454_…`); a guard keeps every citation
//! true: each label a test name leads with is a label of `spec/labels.yaml`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The label prefixes a test name may cite, as they appear in a name.
const PREFIXES: [&str; 9] = ["p", "fg", "f", "rq", "ac", "cmp", "con", "qa", "adr"];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The labels a test name leads with: `ac_165_and_ac_166_…` cites AC-165 and AC-166.
fn cited(name: &str) -> Vec<String> {
    let parts: Vec<&str> = name.split('_').collect();
    let mut labels = Vec::new();
    let mut at = 0;
    loop {
        if parts.get(at) == Some(&"and") && !labels.is_empty() {
            at += 1;
        }
        let (Some(prefix), Some(number)) = (parts.get(at), parts.get(at + 1)) else {
            break;
        };
        if !PREFIXES.contains(prefix) || !number.bytes().all(|b| b.is_ascii_digit()) {
            break;
        }
        labels.push(format!("{}-{number}", prefix.to_uppercase()));
        at += 2;
    }
    labels
}

/// Every label `spec/labels.yaml` binds.
fn labels() -> BTreeSet<String> {
    let text = fs::read_to_string(workspace().join("spec/labels.yaml")).unwrap();
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("label: "))
        .map(str::to_owned)
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

/// Each citation in `names` that `labels` does not bind, as `name: label`.
fn missing(names: &[(String, String)], labels: &BTreeSet<String>) -> Vec<String> {
    names
        .iter()
        .flat_map(|(file, name)| {
            cited(name)
                .into_iter()
                .filter(|label| !labels.contains(label))
                .map(move |label| format!("{file}: {name}: {label}"))
        })
        .collect()
}

#[test]
fn every_label_a_test_name_cites_is_a_label_of_the_spec() {
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
    assert_eq!(missing(&names, &labels()), Vec::<String>::new());
}

#[test]
fn a_citation_is_the_leading_run_of_labels_in_a_name() {
    assert_eq!(cited("ac_454_every_node"), ["AC-454"]);
    assert_eq!(cited("ac_165_and_ac_166_both"), ["AC-165", "AC-166"]);
    assert_eq!(cited("rq_74_ac_317_a_kind"), ["RQ-74", "AC-317"]);
    assert_eq!(cited("a_scenario_round_trips"), Vec::<String>::new());
    assert_eq!(
        cited("and_ac_1_is_not_led_by_a_label"),
        Vec::<String>::new()
    );
}

#[test]
fn a_test_name_citing_a_label_the_spec_does_not_bind_is_reported() {
    let labels = BTreeSet::from(["AC-1".to_owned()]);
    let names = [("t.rs".to_owned(), "ac_1_and_ac_99_x".to_owned())];
    assert_eq!(missing(&names, &labels), ["t.rs: ac_1_and_ac_99_x: AC-99"]);
}

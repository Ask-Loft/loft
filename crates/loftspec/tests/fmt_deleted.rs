//! A deleted node's labels (Labels and aliases; the `loft fmt` Feature): formatting drops the
//! binding of a key no node or decision carries, leaves the counters as they are, and writes a
//! retired or superseded decision's reference to the deleted node as its former label.

use crate::common::text::{FEATURE, files, path, text};
use loftspec::codec::{self, Files};
use loftspec::fmt::format;

const LABELS: &str = "labels.yaml";
const DECISION: &str = "decisions/keys-are-uuids.yaml";
const COMPONENT: &str = "components/document-model.yaml";
const REQUIREMENT: &str = "00000000-0000-7000-8000-000000000004";
const CONSTRAINT: &str = "00000000-0000-7000-8000-000000000007";

/// The files with the text between `from` and `to` in the file at `at` removed, `from`
/// included and `to` kept.
fn cut(files: &Files, at: &str, from: &str, to: &str) -> Files {
    let current = text(files, at);
    let start = current
        .find(from)
        .unwrap_or_else(|| panic!("{from:?} in {at}"));
    let end = start
        + current[start..]
            .find(to)
            .unwrap_or_else(|| panic!("{to:?} in {at}"));
    let mut files = files.clone();
    let edited = format!("{}{}", &current[..start], &current[end..]);
    files.insert(path(at), edited.into_bytes());
    files
}

/// The files with the first `from` replaced by `to` in the file at `at`.
fn replaced(files: &Files, at: &str, from: &str, to: &str) -> Files {
    let current = text(files, at);
    assert!(current.contains(from), "{from:?} in {at}");
    let mut files = files.clone();
    files.insert(path(at), current.replacen(from, to, 1).into_bytes());
    files
}

/// The tree with the Constraint `CON-1` deleted from its Component.
fn without_constraint() -> Files {
    cut(
        &files(),
        COMPONENT,
        "constraints:\n",
        "qualityAttributes:\n",
    )
}

/// The tree with the Requirement `RQ-2` and its criterion deleted from their Feature, and the
/// decision that names it in the state `kind`, with any lines that state needs after it.
fn without_requirement(kind: &str) -> Files {
    let files = cut(&files(), FEATURE, "requirements:\n", "qualityAttributes:\n");
    replaced(
        &files,
        DECISION,
        "  kind: accepted\n",
        &format!("  kind: {kind}\n"),
    )
}

/// The tree after formatting.
fn formatted(files: Files) -> Files {
    let result = format(&files).unwrap();
    let mut files = files;
    files.extend(result.changed);
    files
}

/// The counters block of the bindings file.
fn counters(files: &Files) -> String {
    let labels = text(files, LABELS);
    labels[..labels.find("bindings:").unwrap()].to_owned()
}

#[test]
fn formatting_drops_the_binding_of_a_deleted_node() {
    let files = formatted(without_constraint());
    assert!(!text(&files, LABELS).contains(CONSTRAINT));
    codec::read(&files).unwrap();
}

#[test]
fn formatting_leaves_the_counters_as_they_are() {
    let before = files();
    let after = formatted(without_constraint());
    assert_eq!(counters(&after), counters(&before));
}

#[test]
fn a_binding_whose_key_an_element_carries_is_kept() {
    let before = text(&files(), LABELS);
    let after = text(&formatted(without_constraint()), LABELS);
    let kept = |labels: &str| -> Vec<String> {
        labels
            .lines()
            .filter(|line| line.contains("key:") && !line.contains(CONSTRAINT))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(kept(&after), kept(&before));
    assert!(after.contains("label: RQ-2\n    aliases:\n      - RQ-1\n"));
}

#[test]
fn a_retired_decision_names_a_deleted_node_by_its_former_label() {
    let files = formatted(without_requirement(
        "retired\n  rationale: \"Keys moved to the store.\"",
    ));
    let decision = text(&files, DECISION);
    assert!(!decision.contains(REQUIREMENT), "{decision}");
    let former = "formerLabel: RQ-2\n";
    assert!(
        decision.contains(&format!("    subject:\n      {former}")),
        "the driver: {decision}"
    );
    assert!(
        decision.contains(&format!("  - role: outcome\n    node:\n      {former}")),
        "the subject edge: {decision}"
    );
    let consequence = &decision[decision.find("consequences:").unwrap()..];
    assert!(
        consequence.contains(&format!("    subject:\n      {former}")),
        "the consequence: {decision}"
    );
    assert!(!text(&files, LABELS).contains(REQUIREMENT));
    codec::read(&files).unwrap();
}

#[test]
fn a_superseded_decision_names_a_deleted_node_by_its_former_label() {
    let files = without_requirement("superseded");
    let successor = "  successor: 00000000-0000-7000-8000-000000000014\n";
    let files = replaced(
        &files,
        DECISION,
        "  outcome:",
        &format!("{successor}  outcome:"),
    );
    let decision = text(&formatted(files), DECISION);
    assert!(!decision.contains(REQUIREMENT), "{decision}");
    assert!(decision.contains("formerLabel: RQ-2\n"), "{decision}");
}

#[test]
fn a_live_decision_keeps_the_key_of_a_deleted_node_which_the_reader_refuses() {
    let files = without_requirement("accepted");
    let after = match format(&files) {
        Ok(result) => {
            let mut after = files;
            after.extend(result.changed);
            after
        }
        Err(_) => files,
    };
    assert!(text(&after, DECISION).contains(REQUIREMENT));
    let errors = codec::read(&after).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains(REQUIREMENT)),
        "{errors:?}"
    );
}

#[test]
fn the_formatter_gives_the_label_and_key_of_each_binding_it_drops() {
    let result = format(&without_constraint()).unwrap();
    let dropped: Vec<String> = result
        .dropped
        .iter()
        .map(|(label, key)| format!("{label} {key}"))
        .collect();
    assert_eq!(dropped, [format!("CON-1 {CONSTRAINT}")]);
    assert_eq!(format(&files()).unwrap().dropped, []);
}

//! A deleted node's labels (Labels and aliases; the `loft fmt` Feature): formatting marks
//! departed the binding of a key no node or decision carries, removing none, leaves the counters
//! as they are, writes a retired or superseded decision's reference to the deleted node as its
//! former label, and writes such a reference as the node's key again once the node comes back.

use crate::common::text::{
    DECISION, FEATURE, cut, files, formatted, path, replaced, text, without_constraint,
    without_requirement,
};
use loftspec::codec::{self, Files};
use loftspec::fmt::format;

const LABELS: &str = "labels.yaml";
const REQUIREMENT: &str = "00000000-0000-7000-8000-000000000004";
const CONSTRAINT: &str = "00000000-0000-7000-8000-000000000007";

/// The counters block of the bindings file.
fn counters(files: &Files) -> String {
    let labels = text(files, LABELS);
    labels[..labels.find("bindings:").unwrap()].to_owned()
}

/// The binding of `key` in the bindings file, from its `key` line to the next binding.
fn binding(files: &Files, key: &str) -> String {
    let labels = text(files, LABELS);
    let start = labels.find(&format!("  - key: {key}\n")).unwrap();
    let end = labels[start + 1..]
        .find("  - key: ")
        .map_or(labels.len(), |end| start + 1 + end);
    labels[start..end].to_owned()
}

#[test]
fn formatting_marks_departed_the_binding_of_a_deleted_node() {
    let files = formatted(&without_constraint(&files()));
    let departed = binding(&files, CONSTRAINT);
    assert_eq!(
        departed,
        format!("  - key: {CONSTRAINT}\n    label: CON-1\n    departed: true\n")
    );
    let document = codec::read(&files).unwrap();
    let read = document.bindings.iter().find(|binding| binding.departed);
    assert_eq!(
        read.map(|binding| binding.element.key().to_string()),
        Some(CONSTRAINT.to_owned())
    );
}

#[test]
fn formatting_removes_no_binding() {
    let count = |files: &Files| text(files, LABELS).matches("  - key: ").count();
    let before = files();
    let after = formatted(&without_requirement("retired\n  rationale: \"Gone.\""));
    assert_eq!(count(&after), count(&before));
    assert_eq!(count(&formatted(&after)), count(&before));
}

#[test]
fn a_departed_binding_keeps_its_label_its_aliases_and_its_withdrawn_labels() {
    let files = replaced(
        &without_requirement("retired\n  rationale: \"Gone.\""),
        LABELS,
        "      - RQ-1\n",
        "      - RQ-1\n    withdrawn:\n      - RQ-7\n",
    );
    let departed = binding(&formatted(&files), REQUIREMENT);
    let expected = "    label: RQ-2\n    departed: true\n    aliases:\n      - RQ-1\n    withdrawn:\n      - RQ-7\n";
    assert!(departed.ends_with(expected), "{departed}");
}

#[test]
fn a_binding_already_marked_departed_is_left_as_it_is() {
    let once = formatted(&without_constraint(&files()));
    let again = format(&once).unwrap();
    assert!(again.changed.is_empty(), "{:?}", again.changed.keys());
    assert_eq!(again.departed, []);
}

#[test]
fn a_departed_decision_keeps_the_state_and_history_its_last_publish_recorded() {
    let history = "ab".repeat(32);
    let record = format!("    publishedState: accepted\n    publishedHistory: {history}\n");
    let decision = "00000000-0000-7000-8000-000000000014";
    let mut files = replaced(
        &files(),
        LABELS,
        "    label: ADR-1\n",
        &format!("    label: ADR-1\n{record}"),
    );
    files.remove(&path(DECISION));
    let files = cut(&files, FEATURE, "requirements:\n", "qualityAttributes:\n");
    let departed = binding(&formatted(&files), decision);
    assert!(
        departed.ends_with(&format!("    departed: true\n{record}")),
        "{departed}"
    );
}

#[test]
fn formatting_leaves_the_counters_as_they_are() {
    let before = files();
    let after = formatted(&without_constraint(&files()));
    assert_eq!(counters(&after), counters(&before));
}

#[test]
fn a_binding_whose_key_an_element_carries_is_kept() {
    let before = text(&files(), LABELS);
    let after = text(&formatted(&without_constraint(&files())), LABELS);
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
    let files = formatted(&without_requirement(
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
    assert!(binding(&files, REQUIREMENT).contains("    departed: true\n"));
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
    let decision = text(&formatted(&files), DECISION);
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
fn the_formatter_gives_the_label_and_key_of_each_binding_it_marks_departed() {
    let result = format(&without_constraint(&files())).unwrap();
    let departed: Vec<String> = result
        .departed
        .iter()
        .map(|(label, key)| format!("{label} {key}"))
        .collect();
    assert_eq!(departed, [format!("CON-1 {CONSTRAINT}")]);
    assert_eq!(format(&files()).unwrap().departed, []);
}

#[test]
fn a_node_that_comes_back_is_named_by_its_key_where_its_former_label_stood() {
    let gone = formatted(&without_requirement("retired\n  rationale: \"Gone.\""));
    assert!(text(&gone, DECISION).contains("formerLabel: RQ-2\n"));
    let mut back = gone.clone();
    back.insert(path(FEATURE), files()[&path(FEATURE)].clone());
    let back = formatted(&back);
    let decision = text(&back, DECISION);
    assert!(!decision.contains("formerLabel"), "{decision}");
    assert!(
        decision.contains(&format!("    subject: {REQUIREMENT}\n")),
        "the driver: {decision}"
    );
    assert!(
        decision.contains(&format!("  - role: outcome\n    node: {REQUIREMENT}\n")),
        "the subject edge: {decision}"
    );
    let consequence = &decision[decision.find("consequences:").unwrap()..];
    assert!(
        consequence.contains(&format!("    subject: {REQUIREMENT}\n")),
        "the consequence: {decision}"
    );
    assert!(binding(&back, REQUIREMENT).contains("    departed: true\n"));
    codec::read(&back).unwrap();
}

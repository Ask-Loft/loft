//! The merge of a decision's whole values (F-32): an assessment matched by its option and its
//! driver, a subject by its role and its node, each merged as one value, and a disagreement over
//! an assessment written as one conflict over the decision's assessments.

use crate::common::text::{files, text};
use crate::merge_file::{merge_at, with};

const DECISION: &str = "decisions/keys-are-uuids.yaml";

/// The rationale of the assessment of the second option against the second driver.
const RATIONALE: &str = "rationale: \"Either can be written.\"";

/// The subject that names the Component as affected.
const AFFECTED_COMPONENT: &str =
    "  - role: affected\n    node: 00000000-0000-7000-8000-000000000006\n";

fn decision() -> String {
    text(&files(), DECISION)
}

/// Merges the decision file and gives the result as text.
fn merged(base: &str, ours: &str, theirs: &str) -> (String, usize) {
    let merged = merge_at(DECISION, base, ours, theirs);
    (String::from_utf8(merged.bytes).unwrap(), merged.conflicts)
}

/// The decision's fields before its conflict records.
fn fields(text: &str) -> &str {
    text.find("\nconflicts:\n").map_or(text, |at| &text[..=at])
}

/// `role` and `node`, a subject as the decision file lists it.
fn subject(role: &str, node: &str) -> String {
    format!("  - role: {role}\n    node: 00000000-0000-7000-8000-0000000000{node}\n")
}

/// The decision with the second option's assessment against the second driver given
/// `rationale`.
fn rationale(base: &str, rationale: &str) -> String {
    with(base, &[(RATIONALE, &format!("rationale: \"{rationale}\""))])
}

#[test]
fn assessments_are_matched_by_option_and_driver_and_each_side_s_change_is_taken() {
    let base = decision();
    let ours = rationale(&base, "Ours.");
    let theirs = with(
        &base,
        &[(
            "rationale: \"Two syntaxes in one document.\"",
            "rationale: \"Theirs.\"",
        )],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    let assessment = |driver: &str, rationale: &str| {
        format!(
            "  - option: 00000000-0000-7000-8000-000000000018
    driver: 00000000-0000-7000-8000-0000000000{driver}
    rating: {rationale}\n"
        )
    };
    assert!(
        text.contains(&assessment("16", "neutral\n    rationale: \"Ours.\"")),
        "{text}"
    );
    assert!(
        text.contains(&assessment("15", "conflicts\n    rationale: \"Theirs.\"")),
        "{text}"
    );
    assert_eq!(text.matches("  - option: ").count(), 4, "{text}");
    assert_eq!(conflicts, 0);
}

#[test]
fn an_assessment_both_sides_change_apart_gives_the_decision_one_conflict_over_its_list() {
    let base = decision();
    let (text, conflicts) = merged(
        &base,
        &rationale(&base, "Ours."),
        &rationale(&base, "Theirs."),
    );
    assert_eq!(conflicts, 1, "{text}");
    let (_, records) = text
        .split_once("\nconflicts:\n")
        .expect("a conflict record");
    assert!(records.starts_with("  - field: assessments\n"), "{text}");
    assert_eq!(records.matches("  - field: ").count(), 1, "{text}");
    for side in ["Either can be written.", "Ours.", "Theirs."] {
        assert!(records.contains(&format!("\"{side}\"")), "{side} in {text}");
    }
}

#[test]
fn an_assessment_both_sides_change_apart_keeps_its_base_value() {
    let base = decision();
    let (text, _) = merged(
        &base,
        &rationale(&base, "Ours."),
        &rationale(&base, "Theirs."),
    );
    assert_eq!(fields(&text), base, "{text}");
}

#[test]
fn subjects_are_matched_by_role_and_node() {
    let base = decision();
    let outcome_component = subject("outcome", "06");
    let ours = with(&base, &[(AFFECTED_COMPONENT, &outcome_component)]);
    let affected_requirement = subject("affected", "04");
    let theirs = with(
        &base,
        &[(
            AFFECTED_COMPONENT,
            &format!("{AFFECTED_COMPONENT}{affected_requirement}"),
        )],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    let subjects = text.split_once("subjects:\n").unwrap().1;
    let expected = [
        affected_requirement,
        subject("outcome", "04"),
        outcome_component,
    ]
    .concat();
    assert_eq!(subjects, expected, "{text}");
    assert_eq!(conflicts, 0);
}

#[test]
fn a_subject_each_side_replaces_differently_is_removed_and_both_replacements_are_kept() {
    let base = decision();
    let ours = with(&base, &[(AFFECTED_COMPONENT, &subject("outcome", "06"))]);
    let theirs = with(&base, &[(AFFECTED_COMPONENT, &subject("affected", "03"))]);
    let (text, conflicts) = merged(&base, &ours, &theirs);
    let subjects = text.split_once("subjects:\n").unwrap().1;
    let expected = [
        subject("affected", "03"),
        subject("outcome", "04"),
        subject("outcome", "06"),
    ]
    .concat();
    assert_eq!(subjects, expected, "{text}");
    assert_eq!(conflicts, 0);
}

//! The merge of one file (D-11 F-32..F-34): records matched by key and merged field by field, a
//! conflict written where both sides changed one field apart, lists merged in base order, and
//! the result in canonical layout.

mod common;

use common::text::{FEATURE, files, text};
use loftspec::codec::{self, SpecPath};
use loftspec::merge::{Input, MergeError, Merged, merge_file};

fn feature() -> String {
    text(&files(), FEATURE)
}

/// `text` with each `(from, to)` replaced once.
fn with(text: &str, edits: &[(&str, &str)]) -> String {
    let mut text = text.to_owned();
    for (from, to) in edits {
        assert!(text.contains(from), "{from:?} in\n{text}");
        text = text.replacen(from, to, 1);
    }
    text
}

fn merge_at(path: &str, base: &str, ours: &str, theirs: &str) -> Merged {
    merge_file(
        &SpecPath::new(path),
        base.as_bytes(),
        ours.as_bytes(),
        theirs.as_bytes(),
    )
    .expect("the file merges")
}

/// Merges the Feature file and gives the result as text.
fn merged(base: &str, ours: &str, theirs: &str) -> (String, usize) {
    let merged = merge_at(FEATURE, base, ours, theirs);
    (String::from_utf8(merged.bytes).unwrap(), merged.conflicts)
}

const SHALL: &str = "shall: \"write a key on every node.\"";
const CRITERION: &str = "name: \"Every node record carries a `key` field.\"";
const CRITERION_BLOCK: &str = "      - key: 00000000-0000-7000-8000-000000000005
        name: \"Every node record carries a `key` field.\"
        mentions:
          - 00000000-0000-7000-8000-000000000004
";

#[test]
fn records_are_matched_by_key_and_each_side_s_change_is_taken() {
    let base = feature();
    let ours = with(&base, &[(SHALL, "shall: \"write a key on each node.\"")]);
    let theirs = with(
        &base,
        &[(CRITERION, "name: \"Every record carries a key.\"")],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert!(text.contains("write a key on each node."), "{text}");
    assert!(text.contains("Every record carries a key."), "{text}");
    assert_eq!(conflicts, 0);
}

#[test]
fn a_record_present_on_one_side_only_is_kept() {
    let base = feature();
    let added = "      - key: 00000000-0000-7000-8000-0000000000aa
        name: \"A minted key is version 7.\"
";
    let ours = with(
        &base,
        &[(CRITERION_BLOCK, &format!("{CRITERION_BLOCK}{added}"))],
    );
    let (text, conflicts) = merged(&base, &ours, &base);
    assert!(text.contains(added), "{text}");
    assert_eq!(conflicts, 0);
}

#[test]
fn a_record_deleted_on_one_side_and_unchanged_on_the_other_is_deleted() {
    let base = feature();
    let ours = with(&base, &[("    criteria:\n", ""), (CRITERION_BLOCK, "")]);
    let (text, conflicts) = merged(&base, &ours, &base);
    assert!(!text.contains("000000000005"), "{text}");
    assert_eq!(conflicts, 0);
}

#[test]
fn a_record_deleted_on_one_side_and_changed_on_the_other_is_kept_with_a_record_conflict() {
    let base = feature();
    let ours = with(&base, &[("    criteria:\n", ""), (CRITERION_BLOCK, "")]);
    let theirs = with(
        &base,
        &[(CRITERION, "name: \"Every record carries a key.\"")],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    let kept = "      - key: 00000000-0000-7000-8000-000000000005
        name: \"Every record carries a key.\"
        mentions:
          - 00000000-0000-7000-8000-000000000004
        conflicts:
          - field: record
            base: 00000000-0000-7000-8000-000000000005
            ours: null
            theirs: 00000000-0000-7000-8000-000000000005
";
    assert!(text.contains(kept), "{text}");
    assert_eq!(conflicts, 1);
}

#[test]
fn a_mapping_entry_without_a_key_is_matched_by_its_field_name() {
    let base = feature();
    let ours = with(
        &base,
        &[("system: \"The writer\"", "system: \"The formatter\"")],
    );
    let theirs = with(&base, &[(SHALL, "shall: \"write a key on each node.\"")]);
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert!(text.contains("system: \"The formatter\""), "{text}");
    assert!(
        text.contains("shall: \"write a key on each node.\""),
        "{text}"
    );
    assert_eq!(conflicts, 0);
}

#[test]
fn a_list_element_without_a_key_is_matched_by_its_value() {
    let base = feature();
    let body = "body: \"Keys address nodes.\"\n";
    let ours = with(&base, &[(body, &format!("{body}tags:\n  - identity\n"))]);
    let theirs = with(
        &base,
        &[(body, &format!("{body}tags:\n  - identity\n  - portable\n"))],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert!(
        text.contains("tags:\n  - identity\n  - portable\n"),
        "{text}"
    );
    assert_eq!(conflicts, 0);
}

#[test]
fn the_merged_file_is_in_the_formatter_s_canonical_layout() {
    let base = feature();
    let realizes = "realizes:\n  - 00000000-0000-7000-8000-000000000006\n";
    let ours = with(
        &base,
        &[(
            realizes,
            &format!("{realizes}  - 00000000-0000-7000-8000-000000000001\n"),
        )],
    );
    let (text, _) = merged(&base, &ours, &base);
    let mut files = files();
    files.insert(SpecPath::new(FEATURE), text.clone().into_bytes());
    assert!(
        text.contains(
            "  - 00000000-0000-7000-8000-000000000001\n  - 00000000-0000-7000-8000-000000000006\n"
        ),
        "{text}"
    );
    assert_eq!(
        loftspec::fmt::format(&files).unwrap().changed,
        codec::Files::new()
    );
}

#[test]
fn both_sides_changing_a_field_alike_take_it_once_without_a_conflict() {
    let base = feature();
    let both = with(&base, &[(SHALL, "shall: \"write a key on each node.\"")]);
    let (text, conflicts) = merged(&base, &both, &both);
    assert_eq!(text.matches("write a key on each node.").count(), 1);
    assert_eq!(conflicts, 0);
}

#[test]
fn both_sides_changing_one_field_apart_conflict_on_its_record() {
    let base = feature();
    let ours = with(&base, &[(SHALL, "shall: \"write a key on each node.\"")]);
    let theirs = with(
        &base,
        &[
            (SHALL, "shall: \"write one key on every node.\""),
            (CRITERION, "name: \"Every record carries a key.\""),
        ],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 1);
    assert!(
        text.contains(SHALL),
        "the field keeps its base value: {text}"
    );
    assert!(text.contains("Every record carries a key."), "{text}");
    let conflict = "    conflicts:
      - field: statement.shall
        base: \"write a key on every node.\"
        ours: \"write a key on each node.\"
        theirs: \"write one key on every node.\"
";
    assert!(text.contains(conflict), "{text}");
}

#[test]
fn a_statement_the_merge_would_leave_in_no_form_keeps_its_base_and_conflicts_on_each_change() {
    let base = feature();
    let ours = with(
        &base,
        &[(
            "form: ubiquitous\n",
            "form: state_driven\n      while: \"a key exists\"\n",
        )],
    );
    let theirs = with(
        &base,
        &[(
            "form: ubiquitous\n",
            "form: event_driven\n      when: \"a node is written\"\n",
        )],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert!(text.contains("form: ubiquitous\n"), "{text}");
    assert!(
        !text.contains("      when:") && !text.contains("      while:"),
        "{text}"
    );
    for path in ["statement.form", "statement.when", "statement.while"] {
        assert!(
            text.contains(&format!("field: {path}\n")),
            "{path} in {text}"
        );
    }
    assert_eq!(conflicts, 3);
}

#[test]
fn an_empty_base_is_an_absent_one_and_a_conflicting_field_keeps_ours() {
    let ours = feature();
    let theirs = with(&ours, &[("name: \"Keys\"", "name: \"Key minting\"")]);
    let (text, conflicts) = merged("", &ours, &theirs);
    assert!(text.contains("name: \"Keys\"\n"), "{text}");
    assert!(
        text.contains("  - field: name\n    ours: \"Keys\"\n    theirs: \"Key minting\"\n"),
        "{text}"
    );
    assert_eq!(conflicts, 1);
}

#[test]
fn a_binding_both_sides_change_apart_keeps_its_base_with_a_conflict_over_the_bindings() {
    let base = text(&files(), "labels.yaml");
    let ours = with(&base, &[("label: QA-2", "label: QA-3")]);
    let theirs = with(&base, &[("label: QA-2", "label: QA-4")]);
    let merged = merge_at("labels.yaml", &base, &ours, &theirs);
    let text = String::from_utf8(merged.bytes).unwrap();
    assert!(text.contains("label: QA-2"), "{text}");
    let conflict = "conflicts:
  - field: bindings
    base:
      - key: 00000000-0000-7000-8000-00000000000a
        label: QA-2
    ours:
      - key: 00000000-0000-7000-8000-00000000000a
        label: QA-3
    theirs:
      - key: 00000000-0000-7000-8000-00000000000a
        label: QA-4
";
    assert!(
        text.ends_with(conflict),
        "only the disputed binding: {text}"
    );
    assert_eq!(merged.conflicts, 1);
}

#[test]
fn conflicts_the_inputs_hold_merge_as_a_set() {
    let base = feature();
    let held = "    conflicts:\n      - field: body\n        ours: \"Old.\"\n";
    let after = CRITERION_BLOCK;
    let with_held = with(&base, &[(after, &format!("{after}{held}"))]);
    let (resolved, conflicts) = merged(&with_held, &with_held, &base);
    assert_eq!(
        conflicts, 0,
        "a conflict one side removed is removed: {resolved}"
    );
    let (kept, conflicts) = merged(&base, &with_held, &with_held);
    assert_eq!(
        conflicts, 1,
        "a conflict both sides added is written once: {kept}"
    );
}

#[test]
fn two_conflicts_over_one_field_with_different_sides_are_both_kept() {
    let base = feature();
    let after = CRITERION_BLOCK;
    let held = |side: &str| {
        let conflict = format!("    conflicts:\n      - field: body\n        ours: \"{side}\"\n");
        with(&base, &[(after, &format!("{after}{conflict}"))])
    };
    let (kept, conflicts) = merged(&base, &held("Old."), &held("Other."));
    assert_eq!(
        conflicts, 2,
        "conflicts differing only in a side are two: {kept}"
    );
}

#[test]
fn registries_merge_their_entries_by_key() {
    let base = text(&files(), "tags.yaml");
    let entry = |key: &str, name: &str| {
        format!(
            "- key: 00000000-0000-7000-8000-0000000000{key}\n  name: {name}\n  definition: \"{name}.\"\n"
        )
    };
    let ours = format!("{base}{}", entry("b1", "alpha"));
    let theirs = format!("{base}{}", entry("b2", "omega"));
    let merged = merge_at("tags.yaml", &base, &ours, &theirs);
    let text = String::from_utf8(merged.bytes).unwrap();
    assert!(
        text.contains("name: alpha") && text.contains("name: omega"),
        "{text}"
    );
    assert_eq!(merged.conflicts, 0);
}

#[test]
fn the_lint_configuration_takes_one_side_s_change_and_refuses_two() {
    let base = "rules:\n  - rule: SH-002\n    enabled: false\n";
    let changed = "rules:\n  - rule: SH-003\n    enabled: false\n";
    let merged = merge_at("lint.yaml", base, changed, base);
    assert_eq!(merged.bytes, changed.as_bytes());
    let refused = merge_file(
        &SpecPath::new("lint.yaml"),
        base.as_bytes(),
        changed.as_bytes(),
        b"rules:\n  - rule: LQ-007\n    enabled: false\n",
    );
    assert_eq!(refused, Err(MergeError::BothChanged));
}

#[test]
fn an_input_that_does_not_read_is_named_and_nothing_is_merged() {
    let base = feature();
    let ours = format!("# a comment\n{base}");
    let refused = merge_file(
        &SpecPath::new(FEATURE),
        base.as_bytes(),
        ours.as_bytes(),
        base.as_bytes(),
    );
    let Err(MergeError::Unreadable(errors)) = refused else {
        panic!("refused: {refused:?}");
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, Input::Ours);
}

#[test]
fn a_path_that_names_no_file_of_the_format_is_refused() {
    let refused = merge_file(&SpecPath::new("notes.txt"), b"", b"", b"");
    assert_eq!(refused, Err(MergeError::NotAFormatFile));
}

/// The Feature with a second and a third Requirement after the first.
fn three_requirements() -> String {
    let requirement = |n: &str| {
        format!(
            "  - key: 00000000-0000-7000-8000-0000000000{n}
    statement:
      form: ubiquitous
      system: \"The writer\"
      shall: \"write requirement {n}.\"
"
        )
    };
    with(
        &feature(),
        &[(
            "qualityAttributes:\n",
            &format!(
                "{}{}qualityAttributes:\n",
                requirement("c2"),
                requirement("c3")
            ),
        )],
    )
}

#[test]
fn requirements_both_sides_append_follow_the_base_ones_ours_first() {
    let base = three_requirements();
    let append = |n: &str| {
        with(
            &base,
            &[(
                "qualityAttributes:\n",
                &format!(
                    "  - key: 00000000-0000-7000-8000-0000000000{n}\n    statement:\n      text: \"Requirement {n}.\"\nqualityAttributes:\n"
                ),
            )],
        )
    };
    let (text, conflicts) = merged(&base, &append("d1"), &append("d2"));
    let at = |n: &str| text.find(&format!("0000000000{n}\n")).unwrap();
    assert!(at("c3") < at("d1") && at("d1") < at("d2"), "{text}");
    assert_eq!(conflicts, 0);
}

#[test]
fn a_requirement_moved_on_one_side_and_edited_on_the_other_is_moved_and_edited() {
    let base = three_requirements();
    let third = "  - key: 00000000-0000-7000-8000-0000000000c3\n    statement:\n      form: ubiquitous\n      system: \"The writer\"\n      shall: \"write requirement c3.\"\n";
    let ours = with(
        &base,
        &[
            (third, ""),
            ("requirements:\n", &format!("requirements:\n{third}")),
        ],
    );
    let theirs = with(
        &base,
        &[("write requirement c3.", "write requirement three.")],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    let at = |needle: &str| text.find(needle).unwrap();
    assert!(
        at("0000000000c3") < at("000000000004\n"),
        "moved first: {text}"
    );
    assert!(text.contains("write requirement three."), "{text}");
    assert_eq!(conflicts, 0);
}

#[test]
fn a_requirement_both_sides_moved_apart_stays_with_a_position_conflict() {
    let base = three_requirements();
    let third = "  - key: 00000000-0000-7000-8000-0000000000c3\n    statement:\n      form: ubiquitous\n      system: \"The writer\"\n      shall: \"write requirement c3.\"\n";
    let second = "  - key: 00000000-0000-7000-8000-0000000000c2\n";
    let ours = with(
        &base,
        &[
            (third, ""),
            ("requirements:\n", &format!("requirements:\n{third}")),
        ],
    );
    let theirs = with(&base, &[(third, ""), (second, &format!("{third}{second}"))]);
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 1, "{text}");
    let conflict = "      - field: position
        base: 00000000-0000-7000-8000-0000000000c2
        ours: first
        theirs: 00000000-0000-7000-8000-000000000004
";
    assert!(text.contains(conflict), "{text}");
    let at = |needle: &str| text.find(needle).unwrap();
    assert!(
        at("key: 00000000-0000-7000-8000-0000000000c2")
            < at("key: 00000000-0000-7000-8000-0000000000c3"),
        "{text}"
    );
}

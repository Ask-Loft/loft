//! The merge by key (F-32), which `merge_file` is for record files and registries: deletions
//! beneath a deleted record, moves apart and cycles of moves, assessments, a key carried twice
//! and changes of kind. Each record is merged by its key, its place as one value, and a record
//! one side deleted is kept only where the other side changed it or the merge keeps a record
//! beneath it.

use loftspec::codec::{Problem, SpecPath};
use loftspec::identity::Key;
use loftspec::merge::{MergeError, merge_file};

use crate::common::merge::{
    CRITERIA, CRITERION, read_back, requirement, shall, three_requirements, with,
};
use crate::common::text::{COMPONENT, DECISION, FEATURE, files, indented, text};

const EDITED: &str = "name: \"Every node record carries one key.\"";

/// A second criterion of the first Requirement, after the first.
const SECOND: &str = "      - key: 00000000-0000-7000-8000-0000000000d6
        name: \"A second criterion.\"
";

/// The first Requirement's lines before its criteria.
const FIRST: &str = "  - key: 00000000-0000-7000-8000-000000000004
    statement:
      form: ubiquitous
      system: \"The writer\"
      shall: \"write a key on every node.\"
";

/// A `record` conflict on the record keyed `…0000000000{n}` that ours deleted, indented by
/// `by`.
fn ours_deleted(n: &str, by: &str) -> String {
    let key = format!("00000000-0000-7000-8000-0000000000{n}");
    indented(
        &format!(
            "conflicts:\n  - field: record\n    base: {key}\n    ours: null\n    theirs: {key}\n"
        ),
        by,
    )
}

/// Merges the file at `path` with `merge_file`, which must merge.
fn merged_at(path: &str, base: &str, ours: &str, theirs: &str) -> (String, usize) {
    let merged = merge_file(
        &SpecPath::new(path),
        base.as_bytes(),
        ours.as_bytes(),
        theirs.as_bytes(),
    )
    .expect("the file merges");
    (String::from_utf8(merged.bytes).unwrap(), merged.conflicts)
}

/// Merges the Feature file, and checks that the result reads back.
fn merged(base: &str, ours: &str, theirs: &str) -> (String, usize) {
    let (text, conflicts) = merged_at(FEATURE, base, ours, theirs);
    read_back(FEATURE, &text);
    (text, conflicts)
}

/// The Feature with three Requirements, the first holding two criteria.
fn two_criteria() -> String {
    with(
        &three_requirements(),
        &[(CRITERIA, &format!("{CRITERIA}{SECOND}"))],
    )
}

/// `text` without the first Requirement, which is `requirement` there.
fn without_first(text: &str, requirement: &str) -> String {
    with(text, &[(requirement, "")])
}

/// Ours deletes the first Requirement; theirs edits its first criterion.
fn deleted_and_edited_beneath() -> (String, usize) {
    let base = two_criteria();
    let ours = without_first(&base, &format!("{FIRST}{CRITERIA}{SECOND}"));
    let theirs = with(&base, &[(CRITERION, EDITED)]);
    merged(&base, &ours, &theirs)
}

#[test]
fn a_record_one_side_deletes_is_kept_as_the_other_side_wrote_it_while_a_record_beneath_it_is_kept()
{
    let (text, _) = deleted_and_edited_beneath();
    let edited = with(CRITERIA, &[(CRITERION, EDITED)]);
    let kept = format!("{FIRST}{edited}{}{SECOND}", ours_deleted("05", "        "));
    assert!(text.contains(&kept), "{text}");
}

#[test]
fn a_record_one_side_deletes_and_the_merge_keeps_only_to_hold_a_record_beneath_it_carries_a_record_conflict()
 {
    let (text, conflicts) = deleted_and_edited_beneath();
    let held = format!("{SECOND}{}", ours_deleted("04", "    "));
    assert!(text.contains(&held), "{text}");
    assert_eq!(
        conflicts, 2,
        "the edited criterion and its Requirement: {text}"
    );
}

#[test]
fn a_record_one_side_deletes_and_the_other_changes_only_by_deleting_records_beneath_it_is_deleted()
{
    let base = two_criteria();
    let ours = without_first(&base, &format!("{FIRST}{CRITERIA}{SECOND}"));
    let first = CRITERIA.replace("    criteria:\n", "");
    let theirs = with(&base, &[(&first, "")]);
    // Other files name the deleted Requirement, so the tree is not read back.
    let (text, conflicts) = merged_at(FEATURE, &base, &ours, &theirs);
    assert_eq!(conflicts, 0, "{text}");
    assert!(
        !text.contains("key: 00000000-0000-7000-8000-000000000004"),
        "{text}"
    );
    assert!(!text.contains("0000000000d6"), "{text}");
}

#[test]
fn a_registry_entry_the_sorted_order_moves_when_another_is_renamed_is_unchanged() {
    let entry = |key: &str, name: &str| {
        format!(
            "- key: 00000000-0000-7000-8000-0000000000{key}\n  name: {name}\n  definition: \"{name}.\"\n"
        )
    };
    let (alpha, beta) = (entry("b1", "alpha"), entry("b2", "beta"));
    let gamma = entry("b3", "gamma");
    let base = format!("{alpha}{beta}{gamma}{}", text(&files(), "tags.yaml"));
    // Renaming `alpha` sorts it after `beta`, as if `beta` had moved before it.
    let renamed = entry("b1", "bravo").replace("definition: \"bravo.\"", "definition: \"alpha.\"");
    let ours = with(&base, &[(&alpha, ""), (&beta, &format!("{beta}{renamed}"))]);
    let theirs = with(&base, &[(&beta, "")]);
    let (text, conflicts) = merged_at("tags.yaml", &base, &ours, &theirs);
    assert_eq!(conflicts, 0, "{text}");
    assert!(!text.contains("name: beta"), "{text}");
    assert!(text.starts_with(&format!("{renamed}{gamma}")), "{text}");
}

/// `base` with the criterion moved from the first Requirement to the one keyed `n`.
fn moved_to(base: &str, n: &str) -> String {
    let after = shall(n);
    with(
        base,
        &[(CRITERIA, ""), (&after, &format!("{after}{CRITERIA}"))],
    )
}

#[test]
fn a_record_both_sides_move_apart_whose_base_parent_neither_holds_is_where_ours_put_it_with_a_parent_conflict()
 {
    let base = three_requirements();
    let emptied = |side: &str| with(side, &[(FIRST, "")]);
    let ours = emptied(&moved_to(&base, "c2"));
    let theirs = emptied(&moved_to(&base, "c3"));
    // Other files name the deleted Requirement, so the tree is not read back.
    let (text, conflicts) = merged_at(FEATURE, &base, &ours, &theirs);
    let conflict = "        conflicts:
          - field: parent
            base: 00000000-0000-7000-8000-000000000004
            ours: 00000000-0000-7000-8000-0000000000c2
            theirs: 00000000-0000-7000-8000-0000000000c3
";
    let placed = format!("{}{CRITERIA}{conflict}", shall("c2"));
    assert!(text.contains(&placed), "{text}");
    assert_eq!(conflicts, 1, "{text}");
    assert!(
        !text.contains("key: 00000000-0000-7000-8000-000000000004"),
        "{text}"
    );
}

/// A Section keyed `…0000000000{n}`, at the Feature's top.
fn section(n: &str) -> String {
    format!(
        "  - key: 00000000-0000-7000-8000-0000000000{n}\n    name: \"Section {n}\"\n    body: \"Part {n}.\"\n"
    )
}

/// The Section `outer` holding the Section `inner`.
fn holding(outer: &str, inner: &str) -> String {
    format!("{outer}    sections:\n{}", indented(inner, "    "))
}

#[test]
fn records_whose_moves_would_form_a_cycle_stay_at_the_base_parent_or_where_ours_put_one_base_lacks()
{
    let (held, added) = (section("e1"), section("e9"));
    let base = format!("{}sections:\n{held}", three_requirements());
    // Ours adds `e9` inside `e1`; theirs adds `e9` at the top and moves `e1` inside it.
    let ours = with(&base, &[(&held, &holding(&held, &added))]);
    let theirs = with(&base, &[(&held, &holding(&added, &held))]);
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 2, "{text}");
    let top = "00000000-0000-7000-8000-000000000003";
    let e1 = "00000000-0000-7000-8000-0000000000e1";
    let e9 = "00000000-0000-7000-8000-0000000000e9";
    let inner = format!(
        "{}        conflicts:\n          - field: parent\n            ours: {e1}\n            theirs: {top}\n",
        indented(&added, "    ")
    );
    let outer = format!(
        "{held}    sections:\n{inner}    conflicts:\n      - field: parent\n        base: {top}\n        ours: {top}\n        theirs: {e9}\n"
    );
    assert!(text.ends_with(&format!("sections:\n{outer}")), "{text}");
}

#[test]
fn a_record_both_sides_add_at_different_parents_is_where_ours_put_it_with_a_parent_conflict_without_base()
 {
    let base = three_requirements();
    let added = "      - key: 00000000-0000-7000-8000-0000000000aa
        name: \"A minted key is version 7.\"
";
    let ours = with(&base, &[(CRITERIA, &format!("{CRITERIA}{added}"))]);
    let after = shall("c2");
    let theirs = with(
        &base,
        &[(&after, &format!("{after}    criteria:\n{added}"))],
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    let conflict = "        conflicts:
          - field: parent
            ours: 00000000-0000-7000-8000-000000000004
            theirs: 00000000-0000-7000-8000-0000000000c2
";
    assert!(
        text.contains(&format!("{CRITERIA}{added}{conflict}")),
        "{text}"
    );
    assert_eq!(conflicts, 1, "{text}");
}

/// The assessment of the second option against the second driver.
const ASSESSMENT: &str = "  - option: 00000000-0000-7000-8000-000000000018
    driver: 00000000-0000-7000-8000-000000000016
    rating: neutral
    rationale: \"Either can be written.\"
";

/// Merges the decision file where ours deletes the assessment and theirs changes it.
fn assessment_deleted_and_changed() -> (String, String, usize) {
    let base = text(&files(), DECISION);
    let ours = with(&base, &[(ASSESSMENT, "")]);
    let theirs = with(&base, &[("Either can be written.", "Theirs.")]);
    let (text, conflicts) = merged_at(DECISION, &base, &ours, &theirs);
    (base, text, conflicts)
}

#[test]
fn an_assessment_one_side_deletes_and_the_other_changes_gives_the_decision_a_conflict_over_its_list()
 {
    let (_, text, conflicts) = assessment_deleted_and_changed();
    assert_eq!(conflicts, 1, "{text}");
    let (_, records) = text
        .split_once("\nconflicts:\n")
        .expect("a conflict record");
    assert!(records.starts_with("  - field: assessments\n"), "{text}");
    assert!(records.contains("\"Theirs.\""), "{text}");
}

#[test]
fn an_assessment_one_side_deletes_and_the_other_changes_keeps_its_base_value() {
    let (base, text, _) = assessment_deleted_and_changed();
    let fields = text
        .find("\nconflicts:\n")
        .map_or(&text[..], |at| &text[..=at]);
    assert_eq!(fields, base, "{text}");
}

#[test]
fn a_merge_whose_result_would_carry_a_key_twice_writes_nothing() {
    let base = three_requirements();
    let twice = "00000000-0000-7000-8000-0000000000aa";
    let added = format!("      - key: {twice}\n        name: \"A minted key is version 7.\"\n");
    let ours = with(&base, &[(CRITERIA, &format!("{CRITERIA}{added}"))]);
    let theirs = with(
        &base,
        &[(
            "key: 00000000-0000-7000-8000-000000000003\n",
            &format!("key: {twice}\n"),
        )],
    );
    let refused = merge_file(
        &SpecPath::new(FEATURE),
        base.as_bytes(),
        ours.as_bytes(),
        theirs.as_bytes(),
    );
    let Err(MergeError::Unwritable(error)) = refused else {
        panic!("{refused:?}");
    };
    assert_eq!(
        error.problem,
        Problem::DuplicateKey(Key::parse(twice).unwrap())
    );
    assert_eq!(error.sites.len(), 2, "{error:?}");
}

#[test]
fn a_cycle_through_a_record_the_sides_moved_apart_leaves_each_record_base_holds_at_its_base_parent()
{
    let (outer, inner, other, added) = (section("c1"), section("b1"), section("f1"), section("a1"));
    let base = format!(
        "{}sections:\n{}{other}",
        three_requirements(),
        holding(&outer, &inner)
    );
    // Ours moves `b1` into `f1`. Theirs moves `b1` to the top, adds `a1` inside it and moves
    // `c1` inside `a1`: `b1` is moved apart and stays inside `c1`, which would then sit inside
    // its own record.
    let ours = format!(
        "{}sections:\n{outer}{}",
        three_requirements(),
        holding(&other, &inner)
    );
    let theirs = format!(
        "{}sections:\n{other}{}",
        three_requirements(),
        holding(&inner, &holding(&added, &outer))
    );
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 2, "{text}");
    let parent = |base: &str, ours: &str, theirs: &str| {
        let key = |n: &str| format!("00000000-0000-7000-8000-0000000000{n}");
        format!(
            "conflicts:\n  - field: parent\n    base: {}\n    ours: {}\n    theirs: {}\n",
            key(base),
            key(ours),
            key(theirs)
        )
    };
    let in_inner = format!(
        "{inner}    sections:\n{}{}",
        indented(&added, "    "),
        indented(&parent("c1", "f1", "03"), "    ")
    );
    let tree = format!(
        "{outer}    sections:\n{}{}{other}",
        indented(&in_inner, "    "),
        indented(&parent("03", "03", "a1"), "    ")
    );
    assert!(text.ends_with(&format!("sections:\n{tree}")), "{text}");
}

#[test]
fn a_record_one_side_deletes_and_the_other_moves_within_its_list_is_kept_with_a_record_conflict() {
    let base = three_requirements();
    let c3 = requirement("c3");
    let ours = with(&base, &[(&c3, "")]);
    let theirs = with(&base, &[(&c3, ""), (FIRST, &format!("{c3}{FIRST}"))]);
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 1, "{text}");
    assert!(
        text.contains(&format!(
            "requirements:\n{c3}{}{FIRST}",
            ours_deleted("c3", "    ")
        )),
        "{text}"
    );
}

/// The Component's Constraint.
const CONSTRAINT: &str = "  - key: 00000000-0000-7000-8000-000000000007
    name: \"The document model shall reject unknown kinds.\"
    constraintBinding:
      category: runtime.language
      requiredValue: \"rust\"
    verificationFate: proved
";

/// The Constraint as a Quality Attribute, keeping its key and its name.
const AS_QUALITY: &str = "  - key: 00000000-0000-7000-8000-000000000007
    name: \"The document model shall reject unknown kinds.\"
    qaKind: latency
    qualityTarget:
      metric: \"read.wall-time\"
      band:
        kind: at_most
        upper: \"1\"
      unit: \"s\"
      aggregation: p95
      scenario: \"reading one file\"
      environment: \"the reference developer machine\"
    verificationFate: sampled
";

const RENAMED: &str = "name: \"The document model shall refuse unknown kinds.\"";

/// The Component with its Constraint turned into a Quality Attribute, ahead of the other.
fn as_quality(text: &str) -> String {
    with(
        text,
        &[
            ("constraints:\n", ""),
            (CONSTRAINT, ""),
            (
                "qualityAttributes:\n",
                &format!("qualityAttributes:\n{AS_QUALITY}"),
            ),
        ],
    )
}

/// Merges the Component where theirs turns the Constraint into a Quality Attribute and ours
/// renames it.
fn kind_changed_and_renamed() -> (String, usize) {
    let base = text(&files(), COMPONENT);
    let ours = with(
        &base,
        &[(
            "name: \"The document model shall reject unknown kinds.\"",
            RENAMED,
        )],
    );
    let theirs = as_quality(&base);
    let (text, conflicts) = merged_at(COMPONENT, &base, &ours, &theirs);
    read_back(COMPONENT, &text);
    (text, conflicts)
}

#[test]
fn a_record_one_side_turns_into_another_kind_is_of_that_kind() {
    let (text, conflicts) = kind_changed_and_renamed();
    assert_eq!(conflicts, 0, "{text}");
    assert!(!text.contains("constraints:"), "{text}");
    assert!(
        text.contains("qualityAttributes:\n  - key: 00000000-0000-7000-8000-000000000007\n"),
        "{text}"
    );
    assert!(
        text.contains("    qaKind: latency\n    qualityTarget:\n      metric: \"read.wall-time\""),
        "{text}"
    );
}

#[test]
fn a_field_both_kinds_define_alike_takes_the_other_side_s_change() {
    let (text, _) = kind_changed_and_renamed();
    let quality = AS_QUALITY.replace(
        "name: \"The document model shall reject unknown kinds.\"",
        RENAMED,
    );
    assert!(text.contains(&quality), "{text}");
}

#[test]
fn a_record_the_sides_turn_into_two_different_kinds_keeps_its_base_kind() {
    let base = text(&files(), COMPONENT);
    let ours = as_quality(&base);
    let component = "  - key: 00000000-0000-7000-8000-000000000007
    name: \"The document model shall reject unknown kinds.\"
    body: \"A part of the document model.\"
";
    let theirs = format!(
        "{}components:\n{component}",
        with(&base, &[("constraints:\n", ""), (CONSTRAINT, "")])
    );
    let (text, conflicts) = merged_at(COMPONENT, &base, &ours, &theirs);
    read_back(COMPONENT, &text);
    assert_eq!(conflicts, 0, "{text}");
    // Still a Constraint, with theirs' body and ours' fate, which a Constraint defines as each
    // side's kind does.
    let kept = CONSTRAINT
        .replace(
            "    constraintBinding:",
            "    body: \"A part of the document model.\"\n    constraintBinding:",
        )
        .replace("verificationFate: proved", "verificationFate: sampled");
    assert!(text.contains(&format!("constraints:\n{kept}")), "{text}");
    assert!(!text.contains("components:"), "{text}");
    assert!(!text.contains("read.wall-time"), "{text}");
}

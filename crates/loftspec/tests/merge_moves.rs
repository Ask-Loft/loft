//! A record moved to another parent in one file (F-32): matched with its base record by its
//! key, so that it is written once, at the parent the merge settles on, with the other side's
//! edit, or with a conflict where the sides disagree over its parent or its survival.

use crate::common::merge::{
    CRITERIA, CRITERION, EDITED, merge_at, merged, moved_to, read_back, requirement, shall,
    three_requirements, with,
};
use crate::common::text::{COMPONENT, FEATURE, PRODUCT, files, indented, text};
use loftspec::codec::{Problem, ReadError, Site, SpecPath};
use loftspec::identity::Key;
use loftspec::merge::{Input, MergeError, merge_file};
use loftspec::yaml::{Location, Position};

/// How many records of the text carry `key`.
fn carried(text: &str, key: &str) -> usize {
    text.matches(&format!("key: 00000000-0000-7000-8000-{key}\n"))
        .count()
}

/// Merges the Feature file and asserts that the result holds `conflicts` conflicts, carries the
/// criterion once and reads back; the merged text.
fn merged_once(base: &str, ours: &str, theirs: &str, conflicts: usize) -> String {
    let (text, held) = merged(base, ours, theirs);
    assert_eq!(held, conflicts, "{text}");
    assert_eq!(carried(&text, "000000000005"), 1, "{text}");
    read_back(FEATURE, &text);
    text
}

#[test]
fn a_record_one_side_moves_and_the_other_edits_is_at_its_new_parent_with_the_edit() {
    let base = three_requirements();
    let ours = moved_to(&base, "c2");
    let theirs = with(&base, &[(CRITERION, EDITED)]);
    let text = merged_once(&base, &ours, &theirs, 0);
    let edited = with(CRITERIA, &[(CRITERION, EDITED)]);
    assert!(text.contains(&format!("{}{edited}", shall("c2"))), "{text}");
}

#[test]
fn a_record_moved_into_a_parent_its_side_added_carries_the_other_side_s_edit() {
    let base = three_requirements();
    let added = &requirement("c9");
    let ours = with(
        &base,
        &[
            (CRITERIA, ""),
            (
                "qualityAttributes:\n",
                &format!("{added}{CRITERIA}qualityAttributes:\n"),
            ),
        ],
    );
    let theirs = with(&base, &[(CRITERION, EDITED)]);
    let text = merged_once(&base, &ours, &theirs, 0);
    let edited = with(CRITERIA, &[(CRITERION, EDITED)]);
    assert!(text.contains(&format!("{added}{edited}")), "{text}");
}

#[test]
fn a_record_both_sides_move_to_one_parent_is_there_without_a_conflict() {
    let base = three_requirements();
    let ours = moved_to(&base, "c3");
    let theirs = with(&moved_to(&base, "c3"), &[(CRITERION, EDITED)]);
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 0, "{text}");
    assert!(!text.contains("conflicts:"), "{text}");
    let edited = with(CRITERIA, &[(CRITERION, EDITED)]);
    assert!(text.contains(&format!("{}{edited}", shall("c3"))), "{text}");
    read_back(FEATURE, &text);
}

#[test]
fn a_record_both_sides_move_apart_stays_at_its_base_parent_with_a_parent_conflict() {
    let base = three_requirements();
    let text = merged_once(&base, &moved_to(&base, "c2"), &moved_to(&base, "c3"), 1);
    let first = "      shall: \"write a key on every node.\"\n";
    assert!(text.contains(&format!("{first}{CRITERIA}")), "{text}");
    assert!(text.contains("          - field: parent\n"), "{text}");
}

#[test]
fn each_side_of_a_parent_conflict_holds_the_key_of_the_record_s_parent_on_that_side() {
    let base = three_requirements();
    let (text, _) = merged(&base, &moved_to(&base, "c2"), &moved_to(&base, "c3"));
    let conflict = "        conflicts:
          - field: parent
            base: 00000000-0000-7000-8000-000000000004
            ours: 00000000-0000-7000-8000-0000000000c2
            theirs: 00000000-0000-7000-8000-0000000000c3
";
    assert!(text.contains(&format!("{CRITERIA}{conflict}")), "{text}");
    read_back(FEATURE, &text);
}

#[test]
fn a_record_one_side_moves_and_the_other_deletes_is_kept_where_it_was_moved_with_a_record_conflict()
{
    let base = three_requirements();
    let theirs = with(&base, &[(CRITERIA, "")]);
    let text = merged_once(&base, &moved_to(&base, "c2"), &theirs, 1);
    let conflict = "        conflicts:
          - field: record
            base: 00000000-0000-7000-8000-000000000005
            ours: 00000000-0000-7000-8000-000000000005
            theirs: null
";
    let kept = format!("{}{CRITERIA}{conflict}", shall("c2"));
    assert!(text.contains(&kept), "{text}");
}

#[test]
fn a_new_parent_the_other_side_deletes_is_kept_with_a_record_conflict() {
    let base = three_requirements();
    let second = &requirement("c2");
    let theirs = with(&base, &[(second, ""), (CRITERION, EDITED)]);
    let text = merged_once(&base, &moved_to(&base, "c2"), &theirs, 1);
    let conflict = "    conflicts:
      - field: record
        base: 00000000-0000-7000-8000-0000000000c2
        ours: 00000000-0000-7000-8000-0000000000c2
        theirs: null
";
    let edited = with(CRITERIA, &[(CRITERION, EDITED)]);
    assert!(
        text.contains(&format!("{second}{edited}{conflict}")),
        "the new parent holds the record with the other side's edit: {text}"
    );
}

#[test]
fn the_merged_file_carries_each_key_once() {
    let base = three_requirements();
    let edited = with(&base, &[(CRITERION, EDITED)]);
    let deleted = with(&base, &[(CRITERIA, "")]);
    let apart = moved_to(&base, "c3");
    for theirs in [&edited, &deleted, &apart] {
        let (text, _) = merged(&base, &moved_to(&base, "c2"), theirs);
        for key in [
            "000000000004",
            "000000000005",
            "0000000000c2",
            "0000000000c3",
        ] {
            assert_eq!(carried(&text, key), 1, "{key} in\n{text}");
        }
        read_back(FEATURE, &text);
    }
}

#[test]
fn records_each_side_moves_into_the_other_stay_at_their_base_parents_with_parent_conflicts() {
    let section = |n: &str| {
        format!(
            "  - key: 00000000-0000-7000-8000-0000000000e{n}\n    name: \"Section {n}\"\n    \
             body: \"Part {n}.\"\n"
        )
    };
    let (one, two) = (section("1"), section("2"));
    let inside =
        |outer: &str, inner: &str| format!("{outer}    sections:\n{}", indented(inner, "    "));
    let base = format!("{}sections:\n{one}{two}", three_requirements());
    let ours = with(&base, &[(&two, ""), (&one, &inside(&one, &two))]);
    let theirs = with(&base, &[(&one, ""), (&two, &inside(&two, &one))]);
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 2, "{text}");
    assert_eq!(carried(&text, "0000000000e1"), 1, "{text}");
    assert_eq!(carried(&text, "0000000000e2"), 1, "{text}");
    assert_eq!(text.matches("- field: parent\n").count(), 2, "{text}");
    assert!(
        !text.contains("    sections:\n"),
        "neither is nested: {text}"
    );
    read_back(FEATURE, &text);
}

#[test]
fn a_section_moved_out_of_a_record_the_other_side_moved_is_written_once_with_both_edits() {
    let section = "        sections:
          - key: 00000000-0000-7000-8000-0000000000e5
            name: \"Detail\"
            body: \"Part one.\"
";
    let base = with(
        &three_requirements(),
        &[(CRITERIA, &format!("{CRITERIA}{section}"))],
    );
    let after = shall("c2");
    let ours = with(
        &base,
        &[
            (&format!("{CRITERIA}{section}"), ""),
            (&after, &format!("{after}{CRITERIA}{section}")),
            ("Part one.", "Part 1."),
        ],
    );
    let at_feature = "sections:
  - key: 00000000-0000-7000-8000-0000000000e5
    name: \"Detail\"
    body: \"Part one.\"
";
    let theirs = format!("{}{at_feature}", with(&base, &[(section, "")]));
    let (text, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 0, "{text}");
    assert_eq!(carried(&text, "0000000000e5"), 1, "{text}");
    assert!(text.contains(&format!("{after}{CRITERIA}")), "{text}");
    let at_feature = at_feature.replace("Part one.", "Part 1.");
    assert!(text.ends_with(&at_feature), "{text}");
    read_back(FEATURE, &text);
}

#[test]
fn a_constraint_moved_to_a_nested_component_and_edited_on_the_other_side_is_moved_and_edited() {
    let constraints = "constraints:
  - key: 00000000-0000-7000-8000-000000000007
    name: \"The document model shall reject unknown kinds.\"
    constraintBinding:
      category: runtime.language
      requiredValue: \"rust\"
    verificationFate: proved
";
    let nested = "components:
  - key: 00000000-0000-7000-8000-0000000000d1
    name: \"Reader\"
";
    let base = with(
        &text(&files(), COMPONENT),
        &[(constraints, &format!("{nested}{constraints}"))],
    );
    let moved = format!("{nested}{}", indented(constraints, "    "));
    let ours = with(&base, &[(constraints, ""), (nested, &moved)]);
    let theirs = with(&base, &[("reject unknown kinds.", "refuse unknown kinds.")]);
    let merged = merge_at(COMPONENT, &base, &ours, &theirs);
    let text = String::from_utf8(merged.bytes).unwrap();
    assert_eq!(merged.conflicts, 0, "{text}");
    assert_eq!(carried(&text, "000000000007"), 1, "{text}");
    let expected = moved.replace("reject unknown kinds.", "refuse unknown kinds.");
    assert!(text.contains(&expected), "{text}");
    read_back(COMPONENT, &text);
}

#[test]
fn a_constraint_moved_from_a_feature_group_to_its_product_and_edited_on_the_other_side_is_moved_and_edited()
 {
    let base = text(&files(), PRODUCT);
    let constraint = "  - key: 00000000-0000-7000-8000-000000000009
    name: \"Every Product shall follow the identity rules.\"
    verificationFate: proved
";
    let in_group = format!("    constraints:\n{}", indented(constraint, "    "));
    let ours = format!(
        "{}constraints:\n{constraint}",
        with(&base, &[(&in_group, "")])
    );
    let theirs = with(&base, &[("the identity rules.", "the key rules.")]);
    let merged = merge_at(PRODUCT, &base, &ours, &theirs);
    let text = String::from_utf8(merged.bytes).unwrap();
    assert_eq!(merged.conflicts, 0, "{text}");
    assert_eq!(carried(&text, "000000000009"), 1, "{text}");
    let expected = format!("constraints:\n{}", constraint.replace("identity", "key"));
    assert!(text.ends_with(&expected), "{text}");
    read_back(PRODUCT, &text);
}

#[test]
fn an_input_that_carries_a_key_twice_is_unreadable_at_both_sites() {
    let base = three_requirements();
    let after = shall("c2");
    let ours = with(&base, &[(&after, &format!("{after}{CRITERIA}"))]);
    let refused = merge_file(
        &SpecPath::new(FEATURE),
        base.as_bytes(),
        ours.as_bytes(),
        base.as_bytes(),
    );
    let criterion = "00000000-0000-7000-8000-000000000005";
    let expected = ReadError {
        sites: key_sites(&ours, criterion),
        problem: Problem::DuplicateKey(Key::parse(criterion).unwrap()),
    };
    assert_eq!(expected.sites.len(), 2, "{ours}");
    assert_eq!(
        refused,
        Err(MergeError::Unreadable(vec![(Input::Ours, expected)]))
    );
}

/// The site of each `key` field of the Feature file's text that carries `key`: the line, and
/// the column its value starts at.
fn key_sites(text: &str, key: &str) -> Vec<Site> {
    let field = format!("key: {key}");
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let column = line.find(&field)? + "key: ".len() + 1;
            Some(Site {
                path: SpecPath::new(FEATURE),
                location: Some(Location::At(Position::new(index + 1, column))),
            })
        })
        .collect()
}

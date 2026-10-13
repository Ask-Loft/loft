//! The typed conflict record in the text form (F-34, RQ-220): where it may sit, how its
//! sides are read and written, and what the reader refuses.

mod common;

use common::text::{AFTER_CRITERIA, FEATURE, SHALL_CONFLICT, edited, on_requirement, path, text};
use loftspec::codec::schema::Kind;
use loftspec::codec::{self, Files, Problem, field};
use loftspec::fmt::format;
use loftspec::yaml::{Location, Position};

/// The one read error a tree gives.
fn refused(files: &Files) -> (Problem, Option<Location>) {
    let mut errors = codec::read(files).expect_err("refused");
    assert_eq!(errors.len(), 1, "{errors:?}");
    let error = errors.remove(0);
    (error.problem, error.sites[0].location)
}

/// The files the formatter would rewrite.
fn reformatted(files: &Files) -> Files {
    format(files).expect("the tree formats").changed
}

#[test]
fn a_keyed_record_with_a_conflicts_list_is_read_without_a_read_error() {
    let files = on_requirement(SHALL_CONFLICT);
    assert!(codec::read_tree(&files).is_ok());
    assert!(codec::read(&files).is_ok());
}

#[test]
fn a_conflict_record_outside_a_conflicts_list_is_an_error_naming_the_file_line_and_column() {
    let files = on_requirement("    field: statement.shall\n");
    let (problem, location) = refused(&files);
    assert_eq!(problem, Problem::UnknownField("field".to_owned()));
    assert_eq!(location, Some(Location::At(Position::new(18, 5))));
}

#[test]
fn a_conflict_record_s_fields_are_field_base_ours_theirs_in_that_order() {
    let files = on_requirement(
        "    conflicts:
      - theirs: \"write one key on every node.\"
        ours: \"write a key on each node.\"
        field: statement.shall
        base: \"write a key on every node.\"
",
    );
    let changed = reformatted(&files);
    let written = String::from_utf8(changed[&path(FEATURE)].clone()).unwrap();
    assert!(written.contains(SHALL_CONFLICT), "{written}");
}

#[test]
fn a_conflict_record_requires_field() {
    let files = on_requirement("    conflicts:\n      - ours: \"write a key on each node.\"\n");
    let (problem, _) = refused(&files);
    assert_eq!(problem, Problem::MissingField(field::FIELD));
}

#[test]
fn a_side_other_than_base_ours_or_theirs_is_an_unknown_field_whatever_it_holds() {
    let files = on_requirement(
        "    conflicts:\n      - field: body\n        mine:\n          - \"Why keys.\"\n",
    );
    let (problem, location) = refused(&files);
    assert_eq!(problem, Problem::UnknownField("mine".to_owned()));
    assert_eq!(location, Some(Location::At(Position::new(20, 9))));
}

#[test]
fn a_side_on_which_the_field_is_absent_omits_its_member() {
    let files =
        on_requirement("    conflicts:\n      - field: body\n        ours: \"Why keys.\"\n");
    assert!(codec::read(&files).is_ok());
    assert_eq!(reformatted(&files), Files::new());
}

#[test]
fn each_side_holds_a_value_in_the_shape_the_named_field_takes() {
    let tags = "    conflicts:
      - field: tags
        ours:
          - identity
        theirs:
          - identity
          - portable
";
    assert!(codec::read(&on_requirement(tags)).is_ok());
    let form = "    conflicts:\n      - field: statement.form\n        ours: sometimes\n";
    let (problem, location) = refused(&on_requirement(form));
    assert!(matches!(problem, Problem::Invalid { .. }), "{problem:?}");
    assert_eq!(location, Some(Location::At(Position::new(20, 15))));
}

#[test]
fn a_text_side_is_written_double_quoted() {
    let files = on_requirement(SHALL_CONFLICT);
    assert_eq!(reformatted(&files), Files::new());
}

#[test]
fn a_conflict_over_a_record_s_place_names_the_field_position() {
    let files = on_requirement(
        "    conflicts:
      - field: position
        base: first
        ours: 00000000-0000-7000-8000-000000000005
        theirs: first
",
    );
    assert!(codec::read(&files).is_ok());
    assert_eq!(reformatted(&files), Files::new());
}

#[test]
fn a_keyed_record_carries_conflicts_after_every_other_field() {
    let files = edited(
        FEATURE,
        "    criteria:\n",
        &format!("{SHALL_CONFLICT}    criteria:\n"),
    );
    let changed = reformatted(&files);
    let written = String::from_utf8(changed[&path(FEATURE)].clone()).unwrap();
    let expected = format!("{AFTER_CRITERIA}{SHALL_CONFLICT}");
    assert!(written.contains(&expected), "{written}");
}

#[test]
fn a_record_conflict_holds_the_key_or_null_and_null_is_written_plain() {
    let files = on_requirement(
        "    conflicts:
      - field: record
        base: 00000000-0000-7000-8000-000000000004
        ours: null
        theirs: 00000000-0000-7000-8000-000000000004
",
    );
    assert!(codec::read(&files).is_ok());
    assert_eq!(reformatted(&files), Files::new());
}

#[test]
fn a_quoted_null_is_text_not_a_deletion_and_a_record_side_refuses_it() {
    let files = on_requirement("    conflicts:\n      - field: record\n        ours: \"null\"\n");
    let (problem, _) = refused(&files);
    assert!(matches!(problem, Problem::Invalid { .. }), "{problem:?}");
}

#[test]
fn a_record_side_that_is_neither_a_key_nor_null_is_refused() {
    let files = on_requirement("    conflicts:\n      - field: record\n        ours: gone\n");
    let (problem, _) = refused(&files);
    assert!(matches!(problem, Problem::Invalid { .. }), "{problem:?}");
}

#[test]
fn a_field_path_that_names_no_field_of_the_record_is_refused_at_the_path() {
    let files = on_requirement("    conflicts:\n      - field: statement.nope\n");
    let (problem, location) = refused(&files);
    assert!(matches!(problem, Problem::Invalid { .. }), "{problem:?}");
    assert_eq!(location, Some(Location::At(Position::new(19, 16))));
}

#[test]
fn a_conflict_may_not_hold_keyed_records_in_its_sides() {
    let files = on_requirement("    conflicts:\n      - field: criteria\n");
    let (problem, _) = refused(&files);
    assert!(matches!(problem, Problem::Invalid { .. }), "{problem:?}");
}

#[test]
fn the_manifest_and_the_bindings_record_hold_conflicts_too() {
    let manifest = edited(
        "loftspec.yaml",
        "products:\n",
        "publishBranch: main\nproducts:\n",
    );
    let mut files = manifest;
    let text_of = text(&files, "loftspec.yaml");
    files.insert(
        path("loftspec.yaml"),
        format!("{text_of}conflicts:\n  - field: publishBranch\n    ours: release\n").into_bytes(),
    );
    assert!(codec::read(&files).is_ok());
    let labels = text(&files, "labels.yaml");
    files.insert(
        path("labels.yaml"),
        format!("{labels}conflicts:\n  - field: counters.P\n    ours: 2\n    theirs: 3\n")
            .into_bytes(),
    );
    assert!(codec::read(&files).is_ok());
}

const DECISION: &str = "decisions/keys-are-uuids.yaml";

#[test]
fn a_driver_holds_conflicts_whatever_its_kind() {
    for (after, conflict) in [
        (
            "    statement: \"Converted and new keys share one syntax.\"\n",
            "      - field: label\n        ours: \"One shape\"\n",
        ),
        (
            "    subject: 00000000-0000-7000-8000-000000000004\n",
            "      - field: subject\n        ours: 00000000-0000-7000-8000-000000000006\n",
        ),
    ] {
        let files = edited(
            DECISION,
            after,
            &format!("{after}    conflicts:\n{conflict}"),
        );
        let document = codec::read(&files).expect("the tree reads");
        assert_eq!(document.conflicts.len(), 1, "{conflict}");
    }
}

#[test]
fn a_record_that_holds_no_conflicts_refuses_a_conflicts_list_as_an_unknown_field() {
    let files = edited(
        FEATURE,
        "      shall: \"write a key on every node.\"\n",
        "      shall: \"write a key on every node.\"\n      conflicts:\n        - field: shall\n",
    );
    let (problem, _) = refused(&files);
    assert_eq!(problem, Problem::UnknownField(field::CONFLICTS.to_owned()));
}

#[test]
fn the_names_conflicts_position_and_record_are_reserved_for_conflict_records() {
    let reserved = [field::CONFLICTS, field::POSITION, field::RECORD];
    for kind in Kind::ALL {
        for each in kind.fields() {
            assert!(!reserved.contains(&each.name), "{kind:?}.{}", each.name);
        }
    }
}

#[test]
fn a_conflicted_tree_formats_to_itself() {
    let files = on_requirement(SHALL_CONFLICT);
    assert_eq!(reformatted(&files), Files::new());
}

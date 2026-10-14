//! References written by name (the `loft fmt` Feature; Keys): while formatting, a reference may
//! name its target instead of carrying its key, and the formatter writes the key in its place.

use crate::common::small_document;
use crate::common::text::{FEATURE, PRODUCT, path, text};
use crate::fmt::formatted;
use loftspec::codec::{self, Files, Problem, Site};
use loftspec::fmt::format;
use loftspec::yaml::{Location, Position};

const DECISION: &str = "decisions/keys-are-uuids.yaml";
const COMPONENT: &str = "components/document-model.yaml";
const UUID_V7: &str = "00000000-0000-7000-8000-000000000017";
const ULID: &str = "00000000-0000-7000-8000-000000000018";
const LOCAL_DRIVER: &str = "00000000-0000-7000-8000-000000000015";
const SUBJECT_DRIVER: &str = "00000000-0000-7000-8000-000000000016";
const REQUIREMENT: &str = "00000000-0000-7000-8000-000000000004";
const COMPONENT_KEY: &str = "00000000-0000-7000-8000-000000000006";
const PRODUCT_KEY: &str = "00000000-0000-7000-8000-000000000001";
const REQUIREMENT_NAME: &str = "The writer shall write a key on every node.";

/// The canonical files with every `(from, to)` replacement made in the file at `at`.
fn replaced(files: &Files, at: &str, edits: &[(String, String)]) -> Files {
    let mut current = text(files, at);
    for (from, to) in edits {
        assert!(current.contains(from.as_str()), "{at} holds {from}");
        current = current.replace(from.as_str(), to);
    }
    let mut files = files.clone();
    files.insert(path(at), current.into_bytes());
    files
}

/// `<lead><field>: <key>`, and the same reference written by `name`; `lead` is the indent, or the
/// indent and dash of a list entry whose first field this is.
fn by_name(lead: &str, field: &str, key: &str, name: &str) -> (String, String) {
    let pad = " ".repeat(lead.len());
    (
        format!("{lead}{field}: {key}\n"),
        format!("{lead}{field}:\n{pad}  name: \"{name}\"\n"),
    )
}

/// A list entry `- <key>` at `indent` spaces, and the same entry written by `name`.
fn listed_by_name(indent: usize, key: &str, name: &str) -> (String, String) {
    let pad = " ".repeat(indent);
    (
        format!("{pad}- {key}\n"),
        format!("{pad}- name: \"{name}\"\n"),
    )
}

fn decision_by_name(files: &Files) -> Files {
    let edits = [
        by_name("  ", "selectedOption", UUID_V7, "UUID version 7"),
        by_name("    ", "subject", REQUIREMENT, REQUIREMENT_NAME),
        by_name("  - ", "option", ULID, "ULID"),
        by_name("  - ", "option", UUID_V7, "UUID version 7"),
        by_name("    ", "driver", LOCAL_DRIVER, "One key shape"),
        by_name("    ", "driver", SUBJECT_DRIVER, REQUIREMENT_NAME),
        by_name("    ", "node", COMPONENT_KEY, "Document model"),
        by_name("    ", "node", REQUIREMENT, REQUIREMENT_NAME),
    ];
    replaced(files, DECISION, &edits)
}

#[test]
fn every_reference_written_by_name_formats_to_the_canonical_tree() {
    let canonical = codec::write(&small_document());
    let files = decision_by_name(&canonical);
    let files = replaced(
        &files,
        FEATURE,
        &[
            listed_by_name(2, COMPONENT_KEY, "Document model"),
            listed_by_name(10, REQUIREMENT, REQUIREMENT_NAME),
        ],
    );
    let files = replaced(
        &files,
        COMPONENT,
        &[listed_by_name(2, PRODUCT_KEY, "Example Product")],
    );
    let result = format(&files).unwrap();
    assert_eq!(result.minted, 0);
    let mut formatted = files;
    formatted.extend(result.changed);
    for at in [DECISION, FEATURE, COMPONENT] {
        assert_eq!(text(&formatted, at), text(&canonical, at), "{at}");
    }
}

#[test]
fn a_successor_written_by_name_takes_the_decision_with_that_title() {
    let canonical = codec::write(&small_document());
    let files = replaced(
        &canonical,
        DECISION,
        &[(
            "  kind: accepted\n".to_owned(),
            "  kind: superseded\n  successor:\n    name: \"Keys are UUIDs\"\n".to_owned(),
        )],
    );
    let result = format(&files).unwrap();
    assert!(
        text(&result.changed, DECISION)
            .contains("  successor: 00000000-0000-7000-8000-000000000014\n")
    );
}

#[test]
fn a_reference_by_name_may_name_an_option_minted_in_the_same_run() {
    let canonical = codec::write(&small_document());
    let files = replaced(
        &canonical,
        DECISION,
        &[
            (
                format!("  - key: {ULID}\n    title: \"ULID\"\n"),
                "  - title: \"ULID\"\n".to_owned(),
            ),
            by_name("  ", "selectedOption", UUID_V7, "ULID"),
        ],
    );
    let result = format(&files).unwrap();
    assert_eq!(result.minted, 1);
    let written = text(&result.changed, DECISION);
    let lines: Vec<&str> = written.lines().collect();
    let minted = lines
        .windows(2)
        .find(|pair| pair[1] == "    title: \"ULID\"")
        .and_then(|pair| pair[0].strip_prefix("  - key: "))
        .expect("the option has a key");
    assert!(
        written.contains(&format!("  selectedOption: {minted}\n")),
        "{written}"
    );
}

#[test]
fn only_an_element_of_a_kind_the_field_accepts_answers_a_name() {
    let canonical = codec::write(&small_document());
    let files = replaced(
        &canonical,
        FEATURE,
        &[listed_by_name(2, COMPONENT_KEY, "Example Product")],
    );
    let errors = format(&files).unwrap_err();
    assert_eq!(errors[0].sites[0].path, path(FEATURE));
    assert!(
        errors[0].to_string().contains("Example Product"),
        "{}",
        errors[0]
    );
    let files = replaced(
        &canonical,
        COMPONENT,
        &[listed_by_name(2, PRODUCT_KEY, "Document model")],
    );
    let errors = format(&files).unwrap_err();
    assert_eq!(errors[0].sites[0].path, path(COMPONENT));
    let files = replaced(
        &canonical,
        DECISION,
        &[(
            "  kind: accepted\n".to_owned(),
            "  kind: superseded\n  successor:\n    name: \"Document model\"\n".to_owned(),
        )],
    );
    let errors = format(&files).unwrap_err();
    assert!(errors[0].to_string().contains("decision"), "{}", errors[0]);
}

#[test]
fn a_name_that_matches_nothing_is_an_error_at_the_reference() {
    let canonical = codec::write(&small_document());
    let files = replaced(
        &canonical,
        DECISION,
        &[by_name("  ", "selectedOption", UUID_V7, "Snowflake IDs")],
    );
    let errors = format(&files).unwrap_err();
    // The reference is the record under `selectedOption`, which starts on line 8 at column 5.
    let site = Site {
        path: path(DECISION),
        location: Some(Location::At(Position::new(8, 5))),
    };
    assert_eq!(errors[0].sites, [site]);
    let message = errors[0].to_string();
    assert!(
        message.starts_with(&format!("spec/{DECISION}:8:5:")),
        "{message}"
    );
    assert!(message.contains("Snowflake IDs"), "{message}");
}

#[test]
fn a_name_that_matches_two_elements_names_both_keys() {
    let canonical = codec::write(&small_document());
    let files = replaced(
        &canonical,
        DECISION,
        &[
            (
                "    title: \"ULID\"\n".to_owned(),
                "    title: \"UUID version 7\"\n".to_owned(),
            ),
            by_name("  ", "selectedOption", UUID_V7, "UUID version 7"),
        ],
    );
    let message = format(&files).unwrap_err()[0].to_string();
    assert!(
        message.contains(UUID_V7) && message.contains(ULID),
        "{message}"
    );
}

#[test]
fn a_reference_by_name_reaching_any_other_role_is_refused() {
    let canonical = codec::write(&small_document());
    let files = decision_by_name(&canonical);
    let errors = codec::read(&files).unwrap_err();
    assert_eq!(errors[0].sites[0].path, path(DECISION));
    assert!(errors[0].to_string().contains("loft fmt"), "{}", errors[0]);
}

#[test]
fn an_unresolved_name_changes_no_file() {
    let canonical = codec::write(&small_document());
    let files = replaced(
        &canonical,
        PRODUCT,
        &[(
            "name: \"Example Product\"\n".to_owned(),
            "name: \"Example Product\"\nmentions:\n  - name: \"Nothing\"\n".to_owned(),
        )],
    );
    assert!(format(&files).is_err());
}

#[test]
fn a_reference_by_name_is_a_record_whose_one_field_is_name() {
    let canonical = codec::write(&small_document());
    let (keyed, named) = by_name("  ", "selectedOption", UUID_V7, "UUID version 7");
    let files = replaced(&canonical, DECISION, &[(keyed.clone(), named.clone())]);
    assert_eq!(
        text(&formatted(&files), DECISION),
        text(&canonical, DECISION)
    );
    for (second, field) in [
        (format!("    key: {UUID_V7}\n"), "key"),
        ("    title: \"UUID version 7\"\n".to_owned(), "title"),
    ] {
        let files = replaced(
            &canonical,
            DECISION,
            &[(keyed.clone(), format!("{named}{second}"))],
        );
        let errors = format(&files).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].problem, Problem::UnknownField(field.to_owned()));
    }
}

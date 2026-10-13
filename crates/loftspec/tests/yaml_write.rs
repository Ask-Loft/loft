//! The canonical writer (D-11 F-9): layout, scalar styles, and exact read-back.

#[path = "support/oracle.rs"]
mod oracle;

use loftspec::yaml::{self, Out, Value};
use proptest::prelude::*;

fn text(value: &str) -> Out {
    Out::Text(value.to_owned())
}

fn token(value: &str) -> Out {
    Out::Token(value.to_owned())
}

/// The text the writer gives for one field holding `value`.
fn field(value: Out) -> String {
    yaml::write(&Out::Mapping(vec![("a", value)]))
}

/// What the reader gives back for the one field.
fn read_back(written: &str) -> String {
    let node = yaml::read(written.as_bytes()).unwrap().unwrap();
    let Value::Mapping(entries) = node.value else {
        panic!("a mapping");
    };
    let Value::Scalar(scalar) = &entries[0].value.value else {
        panic!("a scalar");
    };
    scalar.text.clone()
}

#[test]
fn two_space_indentation_and_entries_under_their_key() {
    let document = Out::Mapping(vec![
        ("key", token("0190f5f6-0000-7000-8000-000000000001")),
        (
            "requirements",
            Out::Sequence(vec![
                Out::Mapping(vec![("key", token("k1")), ("name", text("One"))]),
                Out::Mapping(vec![(
                    "criteria",
                    Out::Sequence(vec![Out::Mapping(vec![("name", text("Two"))])]),
                )]),
            ]),
        ),
        ("counters", Out::Mapping(vec![("P", Out::Integer(2))])),
    ]);
    assert_eq!(
        yaml::write(&document),
        "key: 0190f5f6-0000-7000-8000-000000000001\nrequirements:\n  - key: k1\n    name: \"One\"\n  - criteria:\n      - name: \"Two\"\ncounters:\n  P: 2\n"
    );
}

#[test]
fn an_empty_top_level_collection_is_an_empty_file() {
    assert_eq!(yaml::write(&Out::Sequence(Vec::new())), "");
}

#[test]
fn a_single_line_string_is_double_quoted() {
    assert_eq!(field(text("Keys")), "a: \"Keys\"\n");
    assert_eq!(field(text("")), "a: \"\"\n");
}

#[test]
fn a_multi_line_string_is_a_literal_block_with_its_chomping() {
    assert_eq!(field(text("one\ntwo")), "a: |-\n  one\n  two\n");
    assert_eq!(field(text("one\ntwo\n")), "a: |\n  one\n  two\n");
    assert_eq!(
        field(text("one\n\ntwo\n\n\n")),
        "a: |+\n  one\n\n  two\n\n\n"
    );
}

#[test]
fn strings_a_literal_block_would_change_are_double_quoted() {
    assert_eq!(field(text("\n\n")), "a: \"\\n\\n\"\n");
    assert_eq!(field(text("  lead\nx")), "a: \"  lead\\nx\"\n");
    assert_eq!(field(text("\n  lead")), "a: \"\\n  lead\"\n");
    assert_eq!(field(text("a\r\nb")), "a: \"a\\r\\nb\"\n");
    assert_eq!(field(text("a\u{2028}\nb")), "a: \"a\\u2028\\nb\"\n");
}

#[test]
fn the_double_quoted_escapes_are_the_formats() {
    assert_eq!(
        field(text("\\ \" \t \0 \u{7} \u{85} é")),
        "a: \"\\\\ \\\" \\t \\0 \\u0007 \\u0085 é\"\n"
    );
}

#[test]
fn a_token_is_plain_unless_a_yaml_reader_would_take_it_for_something_else() {
    for plain in [
        "RQ-12",
        "event_driven",
        "loftspec/1",
        "runtime.language",
        "p95",
        "0190f5f6-0000-7000-8000-000000000001",
    ] {
        assert_eq!(field(token(plain)), format!("a: {plain}\n"));
    }
    for quoted in [
        "yes", "No", "null", "~", "true", "2024", "1e5", "0x1F", "1:20", "1_000", ".inf", "", "-x",
        "a: b",
    ] {
        assert_eq!(
            field(token(quoted)),
            format!("a: \"{quoted}\"\n"),
            "{quoted}"
        );
    }
    assert_eq!(field(Out::Integer(7)), "a: 7\n");
}

#[test]
fn writing_what_was_read_changes_no_byte() {
    let document = Out::Mapping(vec![
        ("name", text("A\ttab and trailing  ")),
        ("body", text("# Heading\n\n- item  \n\ttabbed\n")),
        ("tags", Out::Sequence(vec![token("portable"), token("yes")])),
    ]);
    let written = yaml::write(&document);
    oracle::agree(&written);
    assert_eq!(
        read_back(&yaml::write(&Out::Mapping(vec![("a", text("x\n"))]))),
        "x\n"
    );
}

proptest! {
    #[test]
    fn any_string_reads_back_byte_for_byte(value in any::<String>()) {
        prop_assert_eq!(read_back(&field(Out::Text(value.clone()))), value);
    }

    #[test]
    fn any_multi_line_text_reads_back_byte_for_byte(
        lines in prop::collection::vec("[ \\t a-z#:\\-\"\\\\]{0,8}", 1..6),
        feeds in 0..4usize,
    ) {
        let mut value = lines.join("\n");
        value.extend(std::iter::repeat_n('\n', feeds));
        let written = field(Out::Text(value.clone()));
        prop_assert_eq!(read_back(&written), value);
        oracle::agree(&written);
    }

    #[test]
    fn any_token_reads_back_as_written(value in "[ -~]{0,12}") {
        prop_assert_eq!(read_back(&field(Out::Token(value.clone()))), value);
    }
}

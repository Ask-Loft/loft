//! The strict reader of the YAML subset (D-11 F-8, F-9, ADR-14).

use loftspec::yaml::{self, Entry, ErrorKind, Location, Node, Position, Style, Value};

fn read(text: &str) -> Node {
    yaml::read(text.as_bytes())
        .unwrap_or_else(|error| panic!("{error}"))
        .expect("a document")
}

fn refused(text: &str) -> (Position, ErrorKind) {
    let error = yaml::read(text.as_bytes()).expect_err("refused");
    match error.location {
        Location::At(position) => (position, error.kind),
        Location::Byte(offset) => panic!("byte {offset}: {}", error.kind),
    }
}

fn entries(node: &Node) -> &[Entry] {
    match &node.value {
        Value::Mapping(entries) => entries,
        other => panic!("not a mapping: {other:?}"),
    }
}

fn items(node: &Node) -> &[Node] {
    match &node.value {
        Value::Sequence(items) => items,
        other => panic!("not a sequence: {other:?}"),
    }
}

fn scalar(node: &Node) -> (Style, &str) {
    match &node.value {
        Value::Scalar(scalar) => (scalar.style, &scalar.text),
        other => panic!("not a scalar: {other:?}"),
    }
}

fn at(line: usize, column: usize) -> Position {
    Position::new(line, column)
}

#[test]
fn ac_112_block_mappings_and_sequences_are_accepted() {
    let node = read(
        "key: 0190f5f6-0000-7000-8000-000000000001\nrequirements:\n  - key: a\n    name: \"N\"\n  - key: b\n",
    );
    let fields = entries(&node);
    assert_eq!(fields[0].key, "key");
    assert_eq!(fields[1].key, "requirements");
    let list = items(&fields[1].value);
    assert_eq!(list.len(), 2);
    assert_eq!(entries(&list[0])[1].key, "name");
    assert_eq!(entries(&list[0])[1].key_position, at(4, 5));
}

#[test]
fn a_sequence_may_start_at_its_key_indentation() {
    let node = read("tags:\n- portable\n- git\nname: \"x\"\n");
    let fields = entries(&node);
    assert_eq!(items(&fields[0].value).len(), 2);
    assert_eq!(fields[1].key, "name");
}

#[test]
fn ac_113_plain_double_quoted_and_literal_scalars_are_accepted() {
    let node =
        read("a: plain\nb: \"quoted \\\"x\\\"\\n\\t\\u00E9\"\nc: |\n  line one\n  line two\n");
    let fields = entries(&node);
    assert_eq!(scalar(&fields[0].value), (Style::Plain, "plain"));
    assert_eq!(
        scalar(&fields[1].value),
        (Style::DoubleQuoted, "quoted \"x\"\n\té")
    );
    assert_eq!(
        scalar(&fields[2].value),
        (Style::Literal, "line one\nline two\n")
    );
}

#[test]
fn literal_chomping_keeps_exactly_the_final_line_feeds_its_header_says() {
    let strip = read("a: |-\n  text\n\n");
    assert_eq!(scalar(&entries(&strip)[0].value).1, "text");
    let clip = read("a: |\n  text\n\n\n");
    assert_eq!(scalar(&entries(&clip)[0].value).1, "text\n");
    let keep = read("a: |+\n  text\n\n\nb: x\n");
    assert_eq!(scalar(&entries(&keep)[0].value).1, "text\n\n\n");
}

#[test]
fn ac_123_ac_124_a_literal_keeps_trailing_spaces_tabs_and_inner_indentation() {
    let node = read("a: |\n  one  \n  \ttab\n    deeper\n\n  after blank\nb: x\n");
    assert_eq!(
        scalar(&entries(&node)[0].value).1,
        "one  \n\ttab\n  deeper\n\nafter blank\n"
    );
}

#[test]
fn an_indentation_indicator_admits_a_first_line_that_begins_with_spaces() {
    let node = read("a: |2-\n    indented\n  plain\n");
    assert_eq!(scalar(&entries(&node)[0].value).1, "  indented\nplain");
}

#[test]
fn a_file_with_no_content_reads_as_nothing() {
    assert_eq!(yaml::read(b"").unwrap(), None);
    assert_eq!(yaml::read(b"\n\n").unwrap(), None);
}

#[test]
fn a_key_without_a_value_is_empty() {
    let node = read("a:\nb: x\n");
    assert_eq!(entries(&node)[0].value.value, Value::Empty);
}

#[test]
fn ac_114_anchors_aliases_and_tags_are_refused_at_their_position() {
    assert_eq!(refused("a: &x 1\n"), (at(1, 4), ErrorKind::Anchor));
    assert_eq!(refused("a: *x\n"), (at(1, 4), ErrorKind::Alias));
    assert_eq!(refused("a: !tag x\n"), (at(1, 4), ErrorKind::Tag));
}

#[test]
fn ac_115_flow_collections_are_refused() {
    assert_eq!(
        refused("a: [1, 2]\n"),
        (at(1, 4), ErrorKind::FlowCollection)
    );
    assert_eq!(
        refused("a:\n  - {b: 1}\n"),
        (at(2, 5), ErrorKind::FlowCollection)
    );
}

#[test]
fn ac_116_folded_and_single_quoted_scalars_are_refused() {
    assert_eq!(refused("a: >\n  x\n"), (at(1, 4), ErrorKind::FoldedScalar));
    assert_eq!(
        refused("a: 'x'\n"),
        (at(1, 4), ErrorKind::SingleQuotedScalar)
    );
}

#[test]
fn ac_117_a_document_marker_is_refused() {
    assert_eq!(
        refused("a: 1\n---\nb: 2\n"),
        (at(2, 1), ErrorKind::DocumentMarker)
    );
    assert_eq!(
        refused("---\na: 1\n"),
        (at(1, 1), ErrorKind::DocumentMarker)
    );
    assert_eq!(
        refused("%YAML 1.2\na: 1\n"),
        (at(1, 1), ErrorKind::DocumentMarker)
    );
}

#[test]
fn ac_248_a_comment_is_refused_wherever_it_stands() {
    assert_eq!(refused("# note\na: 1\n"), (at(1, 1), ErrorKind::Comment));
    assert_eq!(refused("a: 1 # note\n"), (at(1, 6), ErrorKind::Comment));
    assert_eq!(refused("a: \"x\" # note\n"), (at(1, 8), ErrorKind::Comment));
    assert_eq!(
        refused("a:\n  # note\n  b: 1\n"),
        (at(2, 3), ErrorKind::Comment)
    );
    assert_eq!(
        refused("a: |  # note\n  x\n"),
        (at(1, 7), ErrorKind::Comment)
    );
}

#[test]
fn a_hash_inside_content_is_not_a_comment() {
    let node = read("a: C#\nb: \"issue #4\"\nc: |\n  # Heading\n  text\n");
    let fields = entries(&node);
    assert_eq!(scalar(&fields[0].value).1, "C#");
    assert_eq!(scalar(&fields[1].value).1, "issue #4");
    assert_eq!(scalar(&fields[2].value).1, "# Heading\ntext\n");
}

#[test]
fn ac_109_ac_110_a_repeated_field_is_refused_at_its_second_occurrence() {
    assert_eq!(
        refused("a: 1\nb: 2\na: 3\n"),
        (at(3, 1), ErrorKind::DuplicateKey("a".to_owned()))
    );
}

#[test]
fn ac_111_a_tab_in_indentation_is_refused_at_the_tab() {
    assert_eq!(
        refused("a:\n\tb: 1\n"),
        (at(2, 1), ErrorKind::TabInIndentation)
    );
    assert_eq!(
        refused("a:\n  \tb: 1\n"),
        (at(2, 3), ErrorKind::TabInIndentation)
    );
}

#[test]
fn ac_229_ac_230_bytes_that_are_not_utf8_are_refused_at_their_offset() {
    let error = yaml::read(b"a: \xff\n").expect_err("refused");
    assert_eq!(error.location, Location::Byte(3));
    assert_eq!(error.kind, ErrorKind::InvalidUtf8);
}

#[test]
fn ac_231_a_byte_order_mark_is_refused_at_1_1() {
    assert_eq!(
        refused("\u{FEFF}a: 1\n"),
        (at(1, 1), ErrorKind::ByteOrderMark)
    );
}

#[test]
fn a_carriage_return_and_a_control_character_are_refused() {
    assert_eq!(refused("a: 1\r\n"), (at(1, 5), ErrorKind::CarriageReturn));
    assert_eq!(
        refused("a: \u{7}\n"),
        (at(1, 4), ErrorKind::NotPrintable(7))
    );
}

#[test]
fn ac_681_a_column_counts_unicode_scalar_values() {
    assert_eq!(refused("a: \"é\" #\n"), (at(1, 8), ErrorKind::Comment));
}

#[test]
fn structure_the_subset_does_not_define_is_refused() {
    assert_eq!(
        refused("a: 1\n  b: 2\n"),
        (at(2, 3), ErrorKind::MultiLinePlainScalar)
    );
    assert_eq!(
        refused("a: b: c\n"),
        (at(1, 5), ErrorKind::UnexpectedMapping)
    );
    assert_eq!(
        refused("\"a\": 1\n"),
        (at(1, 1), ErrorKind::InvalidKey("\"a\"".to_owned()))
    );
    assert_eq!(refused("? a\n"), (at(1, 1), ErrorKind::ComplexKey));
    assert_eq!(refused("- - a\n"), (at(1, 3), ErrorKind::CompactSequence));
    assert_eq!(refused("a: \"open\n"), (at(1, 4), ErrorKind::UnclosedQuote));
    assert_eq!(
        refused("a: \"\\q\"\n"),
        (at(1, 5), ErrorKind::InvalidEscape)
    );
    assert_eq!(
        refused("a:\n  - x\n  b: 1\n"),
        (at(3, 3), ErrorKind::UnexpectedIndentation)
    );
    assert_eq!(
        refused("  a: 1\n"),
        (at(1, 3), ErrorKind::UnexpectedIndentation)
    );
}

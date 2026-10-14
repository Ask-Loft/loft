//! The trace file (the Feature "The trace file"): its strict reading, and its writing in the
//! document order of its links' nodes.

use crate::common::{key, small_document};
use loftspec::codec::Problem;
use loftspec::trace::{Evidence, FORMAT, Link, Test, Trace, TraceError, read, write};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn errors(text: &str) -> Vec<TraceError> {
    read(Some(text.as_bytes())).expect_err(text)
}

fn one_error(text: &str) -> TraceError {
    let errors = errors(text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    errors.into_iter().next().unwrap()
}

fn link(n: u32, evidence: Evidence) -> Link {
    Link {
        node: key(n),
        reviewed: None,
        evidence,
        position: None,
    }
}

fn tests(names: &[&str]) -> Evidence {
    Evidence::Tests(
        names
            .iter()
            .map(|name| Test {
                file: "tests/a.rs".to_owned(),
                name: (*name).to_owned(),
            })
            .collect(),
    )
}

#[test]
fn no_trace_file_holds_no_link() {
    assert_eq!(read(None), Ok(Trace::default()));
}

#[test]
fn a_link_reads_its_node_review_and_tests() {
    let text = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    reviewed: {DIGEST}\n    tests:\n      - file: \"tests/a.rs\"\n        name: \"a_test\"\n",
        key(5)
    );
    let trace = read(Some(text.as_bytes())).unwrap();
    assert_eq!(trace.links.len(), 1);
    let link = &trace.links[0];
    assert_eq!(link.node, key(5));
    assert_eq!(link.reviewed.unwrap().to_string(), DIGEST);
    assert_eq!(link.evidence, tests(&["a_test"]));
    assert_eq!(link.position.unwrap().line, 3);
}

#[test]
fn a_link_without_reviewed_reads_as_not_reviewed() {
    let text = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    unverified: \"Checked by hand.\"\n",
        key(5)
    );
    let link = &read(Some(text.as_bytes())).unwrap().links[0];
    assert_eq!(link.reviewed, None);
    assert_eq!(
        link.evidence,
        Evidence::Unverified("Checked by hand.".to_owned())
    );
}

#[test]
fn a_field_the_trace_file_does_not_define_is_an_error() {
    let error = one_error("format: loft-trace/1\nlinks:\nowner: me\n");
    assert_eq!(error.problem, Problem::UnknownField("owner".to_owned()));
    assert_eq!(
        error.to_string(),
        "loft.trace.yaml:3:1: the field `owner` is not one the Format defines here"
    );
    let link = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    unverified: \"x\"\n    owner: me\n",
        key(5)
    );
    assert_eq!(
        one_error(&link).problem,
        Problem::UnknownField("owner".to_owned())
    );
}

#[test]
fn a_format_other_than_loft_trace_1_is_an_error() {
    let error = one_error("format: loft-trace/2\nlinks:\n");
    assert!(matches!(error.problem, Problem::Invalid { .. }), "{error}");
    assert_eq!(
        one_error("links:\n").problem,
        Problem::MissingField("format")
    );
}

#[test]
fn a_link_with_both_or_neither_of_tests_and_unverified_is_an_error() {
    let both = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    tests:\n      - file: a\n        name: b\n    unverified: \"x\"\n",
        key(5)
    );
    assert!(one_error(&both).to_string().contains("both"));
    let neither = format!("format: loft-trace/1\nlinks:\n  - node: {}\n", key(5));
    assert!(one_error(&neither).to_string().contains("neither"));
}

#[test]
fn an_empty_tests_list_and_a_blank_reason_are_errors() {
    let empty = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    tests:\n",
        key(5)
    );
    assert!(one_error(&empty).to_string().contains("no test"));
    let blank = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    unverified: \"  \"\n",
        key(5)
    );
    assert!(matches!(one_error(&blank).problem, Problem::Invalid { .. }));
}

#[test]
fn two_links_that_name_one_node_are_an_error() {
    let text = format!(
        "format: loft-trace/1\nlinks:\n  - node: {0}\n    unverified: \"x\"\n  - node: {0}\n    unverified: \"y\"\n",
        key(5)
    );
    let error = one_error(&text);
    assert!(error.to_string().contains("second link"), "{error}");
    assert_eq!(error.to_string().split(':').nth(1), Some("5"));
}

#[test]
fn a_node_that_is_not_a_key_and_a_malformed_reviewed_are_errors() {
    let node = "format: loft-trace/1\nlinks:\n  - node: CON-1\n    unverified: \"x\"\n";
    assert!(one_error(node).to_string().contains("is not a key"));
    let reviewed = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    reviewed: ABC\n    unverified: \"x\"\n",
        key(5)
    );
    assert!(one_error(&reviewed).to_string().contains("hexadecimal"));
}

#[test]
fn a_test_without_a_file_or_with_a_blank_name_is_an_error() {
    let text = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    tests:\n      - name: b\n",
        key(5)
    );
    assert_eq!(one_error(&text).problem, Problem::MissingField("file"));
    let blank = format!(
        "format: loft-trace/1\nlinks:\n  - node: {}\n    tests:\n      - file: a\n        name: \"  \"\n",
        key(5)
    );
    assert!(matches!(one_error(&blank).problem, Problem::Invalid { .. }));
}

#[test]
fn links_are_written_in_the_document_order_of_their_nodes_with_unknown_keys_last() {
    // The small document's order: criterion 5, CON-2 (9), then Component CMP-1's CON-1 (7).
    let trace = Trace {
        links: vec![
            link(900, tests(&["late"])),
            link(7, tests(&["seven"])),
            link(800, tests(&["later"])),
            link(5, Evidence::Unverified("By hand.".to_owned())),
            link(9, tests(&["nine"])),
        ],
    };
    let written = read(Some(write(&trace, &small_document()).as_bytes())).unwrap();
    let order: Vec<_> = written.links.iter().map(|link| link.node).collect();
    assert_eq!(order, [key(5), key(9), key(7), key(800), key(900)]);
}

#[test]
fn a_written_trace_file_reads_back_and_writes_the_same_bytes() {
    let mut reviewed = link(5, tests(&["first", "second test"]));
    reviewed.reviewed = Some(DIGEST.parse().unwrap());
    let trace = Trace {
        links: vec![
            reviewed,
            link(9, Evidence::Unverified("By hand.".to_owned())),
        ],
    };
    let document = small_document();
    let text = write(&trace, &document);
    assert!(
        text.starts_with(&format!("format: {FORMAT}\nlinks:\n")),
        "{text}"
    );
    let again = write(&read(Some(text.as_bytes())).unwrap(), &document);
    assert_eq!(again, text);
}

#[test]
fn a_trace_file_with_no_link_reads_back_empty() {
    let text = write(&Trace::default(), &small_document());
    assert_eq!(read(Some(text.as_bytes())), Ok(Trace::default()));
}

//! `V2-009` (F-21): a Markdown construct in a body outside the Format's subset of paragraphs,
//! lists, code spans, fenced code blocks, emphasis and links; and a body that holds one is read
//! and linted, not refused.

use crate::common::lint::{findings, keys, of, severity};
use crate::common::text::with_feature_body;
use crate::common::{key, small_document};
use loftspec::check::{Checked, check};
use loftspec::codec::Files;
use loftspec::document::Document;
use loftspec::ids::SectionId;
use loftspec::lint::{RuleId, Severity};
use loftspec::tree::Section;

/// The small document with the Feature's body set to `body`.
fn with_body(body: &str) -> Document {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some(body.to_owned());
    document
}

/// The body lines of the `V2-009` findings of `document`, in report order.
fn lines(document: &Document) -> Vec<Option<usize>> {
    of(&findings(document), RuleId::V2009)
        .iter()
        .map(|finding| finding.body_line)
        .collect()
}

#[test]
fn each_construct_outside_the_subset_is_one_v2_009_naming_the_node_and_its_first_line() {
    let mut document = with_body("Intro.\n\n> Quoted\n> on two lines.\n\n> ~~Old~~ text.\n");
    document.products[0].feature_groups[0].features[0]
        .details
        .sections
        .push(Section {
            id: Some(SectionId::new(key(70))),
            name: "Notes".to_owned(),
            body: "Text.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n".to_owned(),
            sections: Vec::new(),
            ext: loftspec::ext::Ext::new(),
        });
    let all = findings(&document);
    let found: Vec<_> = of(&all, RuleId::V2009)
        .into_iter()
        .map(|finding| (keys(finding), finding.body_line))
        .collect();
    // The second block quote holds struck-through text: nested constructs each count.
    assert_eq!(
        found,
        [
            (vec![key(3)], Some(3)),
            (vec![key(3)], Some(6)),
            (vec![key(3)], Some(6)),
            (vec![key(70)], Some(3)),
        ]
    );
}

#[test]
fn raw_html_as_a_block_or_inline_is_v2_009() {
    let block = "Intro.\n\n<div>\nA block.\n</div>\n";
    assert_eq!(lines(&with_body(block)), [Some(3)]);
    let inline = "Intro.\n\nA <br> break.\n";
    assert_eq!(lines(&with_body(inline)), [Some(3)]);
}

#[test]
fn each_listed_construct_is_outside_the_subset() {
    for (construct, body) in [
        ("a block quote", "Text.\n\n> Quoted."),
        ("a table", "Text.\n\n| a | b |\n|---|---|\n| 1 | 2 |"),
        ("an image", "Text.\n\nSee ![a chart](chart.png)."),
        ("a thematic break", "Text.\n\n***\n\nMore."),
        ("an indented code block", "Text.\n\n    let x = 1;"),
        ("a footnote", "Text.\n\n[^1]: A note."),
        ("a task list item", "Text.\n\n- [ ] Write it."),
        ("struck-through text", "Text.\n\nIt is ~~old~~ new."),
    ] {
        assert_eq!(lines(&with_body(body)), [Some(3)], "{construct}");
    }
}

#[test]
fn a_heading_is_not_v2_009() {
    let document = with_body("Intro.\n\n## Detail\n\nText.\nUnderlined\n----------\n");
    assert_eq!(of(&findings(&document), RuleId::V2001).len(), 2);
    assert_eq!(lines(&document), []);
}

#[test]
fn text_inside_a_fenced_code_block_or_a_code_span_is_not_v2_009() {
    let body = "Intro.\n\n```\n<div>\n> quoted\n| a | b |\n|---|---|\n    indented\n***\n```\n\n\
                The `<br>`, `~~old~~` and `![a](b.png)` spans.\n";
    assert_eq!(lines(&with_body(body)), []);
}

#[test]
fn a_body_written_only_in_the_subset_is_not_v2_009() {
    let body = "A paragraph with *emphasis*, **strong** text, a `code span`, a\n\
                [link](https://example.com) and <https://example.com>.  \n\
                A hard break came before this line.\n\n\
                - A bullet\n  - nested\n- Another\n\n\
                1. First\n2. Second\n\n\
                ```rust\nlet x = 1;\n```\n";
    assert_eq!(lines(&with_body(body)), []);
    assert_eq!(lines(&small_document()), []);
}

#[test]
fn v2_009_is_info_by_default() {
    assert_eq!(severity(RuleId::V2009), Severity::Info);
}

/// The rules of the findings a tree checks with, after asserting it read and validated.
fn checked_rules(files: &Files) -> Vec<RuleId> {
    match check(files) {
        Checked::Read {
            invalid,
            config,
            findings,
        } => {
            assert!(invalid.is_empty() && config.is_empty(), "{invalid:?}");
            findings.iter().map(|finding| finding.rule).collect()
        }
        Checked::Unread(errors) => panic!("refused: {errors:?}"),
    }
}

#[test]
fn raw_html_in_a_body_is_a_lint_finding_not_a_read_error() {
    let files = with_feature_body("Keys <b>address</b> nodes.\n\n<div>\nA block.\n</div>");
    let rules = checked_rules(&files);
    assert_eq!(
        rules.iter().filter(|rule| **rule == RuleId::V2009).count(),
        3
    );
}

#[test]
fn any_other_construct_in_a_body_is_a_lint_finding_not_a_read_error() {
    let files =
        with_feature_body("Keys address nodes.\n\n> Quoted.\n\n| a | b |\n|---|---|\n| 1 | 2 |");
    let rules = checked_rules(&files);
    assert_eq!(
        rules.iter().filter(|rule| **rule == RuleId::V2009).count(),
        2
    );
}

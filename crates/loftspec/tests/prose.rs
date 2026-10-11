//! What the Format reads in prose (D-11 F-14, F-21): label references (RQ-53) and Markdown
//! heading lines in a body (RQ-54, RQ-100).

use loftspec::identity::Label;
use loftspec::prose::{heading_lines, label_references};

fn labels(text: &str) -> Vec<String> {
    label_references(text)
        .into_iter()
        .map(|label: Label| label.to_string())
        .collect()
}

#[test]
fn ac_188_every_label_form_is_recognised() {
    assert_eq!(
        labels("P-1, FG-2, F-3, RQ-4, AC-5, CMP-6, CON-7, QA-8 and ADR-9."),
        [
            "P-1", "FG-2", "F-3", "RQ-4", "AC-5", "CMP-6", "CON-7", "QA-8", "ADR-9"
        ]
    );
}

#[test]
fn ac_189_a_reference_with_a_document_prefix_is_recognised() {
    assert_eq!(
        labels("As D-1/RQ-12 and D-11/AC-3 state."),
        ["RQ-12", "AC-3"]
    );
}

#[test]
fn a_reference_in_backticks_or_brackets_is_recognised() {
    assert_eq!(
        labels("See `RQ-12` (AC-3) [ADR-4]."),
        ["RQ-12", "AC-3", "ADR-4"]
    );
}

#[test]
fn a_label_inside_a_longer_word_or_number_is_not_a_reference() {
    assert!(labels("XRQ-1 RQ-1a RQ-12b BIG-RQ-3 RQ-01 FT-3 rq-4 RQ-").is_empty());
    assert_eq!(labels("F-1.5 is a version, not F-1"), ["F-1"]);
}

#[test]
fn ac_406_an_atx_heading_line_is_found_with_its_line_number() {
    assert_eq!(heading_lines("Intro.\n# Title\ntext\n### Deep"), [2, 4]);
    assert_eq!(heading_lines("   ## Indented three"), [1]);
}

#[test]
fn a_setext_heading_is_found_at_its_text_line() {
    assert_eq!(heading_lines("Title\n=====\n\nSub\n---\nmore"), [1, 4]);
}

#[test]
fn ac_407_text_that_only_resembles_a_heading_is_not_one() {
    assert!(heading_lines("#hashtag\n    # indented code\n\n---\n| a | b |\n|---|---|").is_empty());
    assert!(heading_lines("```\n# in a fence\n```\n~~~\n## also\n~~~").is_empty());
    assert!(heading_lines("").is_empty());
}

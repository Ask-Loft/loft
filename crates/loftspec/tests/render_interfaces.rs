//! Interfaces in the Markdown projection: their own section after the Components, each rendered
//! with its Requirements as a Feature is, and each Component's `provides` and `requires` lines.

use crate::common::interfaces::provided_and_required;
use crate::common::small_document;
use loftspec::document::Document;
use loftspec::publish::allocate;
use loftspec::render::render;

/// The small document, published, whose root Component provides "Document lookup" (`IF-1`) and
/// requires "Zeta feed" (`IF-2`).
fn linked() -> Document {
    let mut document = provided_and_required();
    allocate(&mut document);
    document
}

fn markdown(document: &Document) -> String {
    render(document).unwrap().markdown
}

fn at(markdown: &str, text: &str) -> usize {
    markdown
        .find(text)
        .unwrap_or_else(|| panic!("no {text:?} in\n{markdown}"))
}

#[test]
fn interfaces_render_in_their_own_section_after_the_components() {
    let markdown = markdown(&linked());
    let order = [
        at(&markdown, "## Component Model\n"),
        at(&markdown, "## Interfaces\n"),
        at(&markdown, "### IF-1 · Document lookup\n"),
        at(&markdown, "### IF-2 · Zeta feed\n"),
        at(&markdown, "## Architecture Decisions\n"),
    ];
    assert!(order.is_sorted(), "{order:?}\n{markdown}");
}

#[test]
fn an_interfaces_requirements_and_criteria_render_beneath_it_as_a_features_do() {
    let markdown = markdown(&linked());
    let order = [
        at(
            &markdown,
            "### IF-1 · Document lookup\n\nWhat a reader of the document may rely on.\n",
        ),
        at(
            &markdown,
            "#### RQ-3 · The lookup shall answer a document by its key.\n",
        ),
        at(
            &markdown,
            "**AC-2 · Acceptance Criterion · The answer names the document.**\n",
        ),
        at(&markdown, "### IF-2 · Zeta feed\n"),
    ];
    assert!(order.is_sorted(), "{order:?}\n{markdown}");
}

#[test]
fn a_components_provides_and_requires_render_as_lines_naming_each_interface() {
    let markdown = markdown(&linked());
    let order = [
        at(&markdown, "### CMP-1 · Document model\n"),
        at(&markdown, "Provides: IF-1 · Document lookup\n"),
        at(&markdown, "Requires: IF-2 · Zeta feed\n"),
        at(&markdown, "## Interfaces\n"),
    ];
    assert!(order.is_sorted(), "{order:?}\n{markdown}");
}

#[test]
fn a_document_without_an_interface_renders_no_interface_section_or_line() {
    let markdown = markdown(&small_document());
    for text in ["## Interfaces", "Provides:", "Requires:"] {
        assert!(!markdown.contains(text), "{text}\n{markdown}");
    }
}

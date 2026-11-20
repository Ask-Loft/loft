//! `loft reconcile` over a tree whose Shelf store provides another document's Interface, which
//! a copy holds: the copied criterion is in scope, named by its handle and label, and its link
//! is accepted by that name.

use std::fs;

use crate::common::copies::{HANDLE, copy_lookup};
use crate::common::stderr;
use crate::reconcile::{findings, link, project, trace_text, write_trace};

/// The copied criterion's key.
const ANSWER: &str = "00000000-0000-4000-8000-0000000000b2";
const STORE: &str = "components/shelf-store.yaml";

/// The Reading List with a copy of the other document's "Shelf lookup", which the Shelf store
/// provides.
fn answering() -> crate::common::Scratch {
    let scratch = project();
    copy_lookup(&scratch);
    let lookup = crate::interfaces::LOOKUP;
    scratch.edit(
        STORE,
        "scopedTo:\n",
        &format!("provides:\n  - document: {HANDLE}\n    key: {lookup}\nscopedTo:\n"),
    );
    let formatted = scratch.loft(&["fmt"]);
    assert_eq!(formatted.status.code(), Some(0), "{}", stderr(&formatted));
    scratch
}

/// The label the copy binds to the copied criterion, as `<handle>:<label>`.
fn cited(scratch: &crate::common::Scratch) -> String {
    let copy = fs::read_to_string(scratch.file("documents/shelves/shelf-lookup.yaml")).unwrap();
    let at = copy.find(&format!("key: {ANSWER}\n")).unwrap();
    let label = copy[at..]
        .lines()
        .nth(1)
        .unwrap()
        .trim()
        .strip_prefix("label: ")
        .unwrap();
    format!("{HANDLE}:{label}")
}

#[test]
fn a_provided_copied_criterion_no_link_names_is_reported_by_its_handle_and_label() {
    let scratch = answering();
    let output = scratch.loft(&["reconcile"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let name = cited(&scratch);
    assert!(name.starts_with("shelves:AC-"), "{name}");
    assert!(findings(&output).contains(&("TR-001".to_owned(), name)));
}

#[test]
fn accepting_a_handle_and_label_accepts_the_link_to_that_foreign_criterion() {
    let scratch = answering();
    write_trace(&scratch, &[link(ANSWER, "a_book_is_added")]);
    let name = cited(&scratch);
    let output = scratch.loft(&["reconcile", "--accept", &name]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(crate::common::stdout(&output).contains(&format!("accepted {name}")));
    assert!(
        trace_text(&scratch).contains("reviewed: "),
        "{}",
        trace_text(&scratch)
    );
}

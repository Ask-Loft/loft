//! `loft reconcile` with the test locator (the Features "Locating a test" and "Reconciliation
//! findings"): accepting records each located test's digest as its `body` in a `loft-trace/2`
//! file, a test rewritten after it was recorded is `TR-006`, a test found only as a word is
//! `TR-007`, and the JSON document lists each test and its locations.

use std::fs;

use crate::common::stdout;
use crate::reconcile::{
    AC_1, AC_2, TEST_FILE, findings, json_of, link, link_in, project, trace_text, write_trace,
};
use serde_json::json;
use sha2::{Digest as _, Sha256};

/// The digest of a test whose canonical text is `canonical`.
fn digest_of(canonical: &str) -> String {
    format!("{:x}", Sha256::digest(canonical))
}

#[test]
fn accepting_writes_loft_trace_2_with_the_body_of_each_located_test() {
    let scratch = project();
    write_trace(
        &scratch,
        &[link(AC_1, "a_book_is_added"), link(AC_2, "a_book_is_added")],
    );
    let output = scratch.loft(&["reconcile", "--accept", "AC-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    let written = trace_text(&scratch);
    assert!(written.starts_with("format: loft-trace/2\n"), "{written}");
    let body = format!(
        "        body: {}\n",
        digest_of("fn a_book_is_added ( ) { }")
    );
    // AC-2's link is not accepted, but its located test is recorded.
    assert_eq!(written.matches(&body).count(), 2, "{written}");
    assert_eq!(written.matches("reviewed:").count(), 1, "{written}");
}

#[test]
fn a_test_rewritten_after_it_was_recorded_is_tr_006_until_accepted_again() {
    let scratch = project();
    write_trace(&scratch, &[link(AC_1, "a_book_is_added")]);
    scratch.loft(&["reconcile", "--accept", "AC-1"]);
    let test_file = scratch.0.join(TEST_FILE);
    fs::write(&test_file, "fn a_book_is_added()\n{\n}\n").unwrap();
    let respaced = scratch.loft(&["reconcile", "--fail-on", "warning"]);
    assert_eq!(respaced.status.code(), Some(0), "{}", stdout(&respaced));
    fs::write(&test_file, "fn a_book_is_added() { assert!(true); }\n").unwrap();
    let changed = scratch.loft(&["reconcile", "--fail-on", "warning"]);
    assert_eq!(changed.status.code(), Some(1), "{}", stdout(&changed));
    assert!(findings(&changed).contains(&("TR-006".to_owned(), "AC-1".to_owned())));
    assert!(stdout(&changed).contains(
        "warning TR-006: AC-1: `a_book_is_added` in `tests/shelf.rs` changed after it was recorded"
    ));
    let accepted = scratch.loft(&["reconcile", "--accept-stale", "--fail-on", "warning"]);
    assert_eq!(accepted.status.code(), Some(0), "{}", stdout(&accepted));
    assert!(stdout(&accepted).contains("accepted AC-1"));
}

#[test]
fn a_test_found_only_as_a_word_in_a_script_is_tr_007() {
    let scratch = project();
    fs::write(
        scratch.0.join("tests/flows.mjs"),
        "check(\"the shelf opens\", ok);\n",
    )
    .unwrap();
    write_trace(
        &scratch,
        &[link_in(AC_1, "tests/flows.mjs", "\"the shelf opens\"")],
    );
    let output = scratch.loft(&["reconcile"]);
    assert!(
        stdout(&output).contains(
            "info TR-007: AC-1: `the shelf opens` is found in `tests/flows.mjs` only as a whole \
             word, so its content is not tracked: no test in the file carries the name"
        ),
        "{}",
        stdout(&output)
    );
}

#[test]
fn the_json_document_lists_each_test_with_its_locations() {
    let scratch = project();
    fs::write(
        scratch.0.join(TEST_FILE),
        "\n#[test]\nfn a_book_is_added() {\n}\n",
    )
    .unwrap();
    write_trace(
        &scratch,
        &[
            link(AC_1, "a_book_is_added"),
            link_in(AC_2, "tests/notes.txt", "a_book_is_added"),
        ],
    );
    fs::write(scratch.0.join("tests/notes.txt"), "a_book_is_added\n").unwrap();
    let document = json_of(&scratch.loft(&["reconcile", "--format", "json"]));
    assert_eq!(
        document["tests"],
        json!([
            {
                "key": AC_1,
                "file": TEST_FILE,
                "name": "a_book_is_added",
                "located": true,
                "locations": [{ "firstLine": 2, "lastLine": 4 }],
            },
            {
                "key": AC_2,
                "file": "tests/notes.txt",
                "name": "a_book_is_added",
                "located": false,
            },
        ])
    );
}

/// The project with AC-1's link reviewed in a `loft-trace/1` file, its test with no `body`.
fn reviewed_without_bodies() -> crate::common::Scratch {
    let scratch = project();
    write_trace(&scratch, &[link(AC_1, "a_book_is_added")]);
    scratch.loft(&["reconcile", "--accept", "AC-1"]);
    let mut reviewed = String::new();
    for line in trace_text(&scratch).lines() {
        if !line.trim_start().starts_with("body:") {
            reviewed.push_str(line);
            reviewed.push('\n');
        }
    }
    let first = reviewed.replacen("format: loft-trace/2", "format: loft-trace/1", 1);
    fs::write(scratch.0.join("loft.trace.yaml"), first).unwrap();
    scratch
}

#[test]
fn accepting_the_stale_links_records_the_bodies_of_a_reviewed_trace_file() {
    let scratch = reviewed_without_bodies();
    let output = scratch.loft(&["reconcile", "--accept-stale"]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert_eq!(
        stdout(&output).lines().next(),
        Some("recorded the body of 1 test")
    );
    assert!(!stdout(&output).contains("accepted "));
    let written = trace_text(&scratch);
    assert!(written.starts_with("format: loft-trace/2\n"), "{written}");
    let body = digest_of("fn a_book_is_added ( ) { }");
    assert!(
        written.contains(&format!("        body: {body}\n")),
        "{written}"
    );
}

#[test]
fn the_json_document_carries_the_number_of_bodies_recorded() {
    let scratch = reviewed_without_bodies();
    let document = json_of(&scratch.loft(&["reconcile", "--accept-stale", "--format", "json"]));
    assert_eq!(document["recorded"], json!(1));
    assert_eq!(document["accepted"], json!([]));
    let again = json_of(&scratch.loft(&["reconcile", "--format", "json"]));
    assert_eq!(again["recorded"], json!(0));
}

#[test]
fn rewrapping_a_python_call_raises_no_tr_006() {
    let scratch = project();
    let test_file = scratch.0.join("tests/test_shelf.py");
    fs::write(&test_file, "def test_added():\n    assert add(1, 2) == 3\n").unwrap();
    write_trace(
        &scratch,
        &[link_in(AC_1, "tests/test_shelf.py", "test_added")],
    );
    scratch.loft(&["reconcile", "--accept", "AC-1"]);
    fs::write(
        &test_file,
        "def test_added():\n    assert add(\n        1,\n        2\n    ) == 3\n",
    )
    .unwrap();
    let rewrapped = scratch.loft(&["reconcile", "--fail-on", "warning"]);
    assert_eq!(rewrapped.status.code(), Some(0), "{}", stdout(&rewrapped));
    fs::write(&test_file, "def test_added():\n    assert add(1, 2) == 4\n").unwrap();
    let changed = scratch.loft(&["reconcile", "--fail-on", "warning"]);
    assert!(findings(&changed).contains(&("TR-006".to_owned(), "AC-1".to_owned())));
}

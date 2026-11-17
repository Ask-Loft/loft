//! `loft status --base` over the decisions and the edges a record writes (the `loft status`
//! Feature): a decision added, changed or removed is listed after the nodes of its list, a
//! Feature whose `realizes` entries changed is listed as changed, and a decision that differs only
//! in naming a node by its former label is not listed, over the Reading List.

use std::fs;

use crate::base::based_on_now;
use crate::common::{Scratch, stdout};
use crate::status::{json, keys, status};
use serde_json::Value;

/// The retired decision Keep a change log, ADR-5, which names RQ-2 in a consequence and as its
/// outcome.
const CHANGE_LOG: &str = "decisions/keep-a-change-log.yaml";
const CHANGE_LOG_KEY: &str = "00000000-0000-4000-8000-00000000008c";
/// The accepted decision Store a shelf in one file, ADR-1, which names RQ-2 in a driver and as
/// its outcome.
const STORE_A_SHELF: &str = "decisions/store-a-shelf-in-one-file.yaml";
/// The Feature Add a book, F-1, which realizes the Shelf store and holds RQ-2.
const ADD_A_BOOK: &str = "products/reading-list/shelf/add-a-book.yaml";
const ADD_A_BOOK_KEY: &str = "00000000-0000-4000-8000-00000000001e";
/// Requirement RQ-2 of Add a book, and its criterion AC-3.
const KEEP_ORDER_KEY: &str = "00000000-0000-4000-8000-00000000002b";
const IN_ORDER_KEY: &str = "00000000-0000-4000-8000-00000000002c";
/// A decision the base does not hold.
const SORT_BY_TITLE: &str = "decisions/sort-books-by-title.yaml";
const SORT_BY_TITLE_KEY: &str = "00000000-0000-4000-8000-0000000000c1";
/// A criterion the base does not hold, added under AC-3.
const ADDED_KEY: &str = "00000000-0000-4000-8000-0000000000aa";

fn compared(scratch: &Scratch, git: &software_plant::GitScenario) -> Value {
    json(&status(
        scratch,
        &["--base", "develop", "--format", "json"],
        git,
    ))
}

#[test]
fn an_added_decision_is_listed_after_the_added_nodes_with_its_kind_title_label_and_site() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.edit(
        ADD_A_BOOK,
        "        name: \"Books list in the order they were added.\"\n",
        &format!(
            "        name: \"Books list in the order they were added.\"\n      - key: {ADDED_KEY}\n        name: \"A book added twice lists once.\"\n"
        ),
    );
    fs::write(
        scratch.file(SORT_BY_TITLE),
        format!(
            "key: {SORT_BY_TITLE_KEY}\nslug: sort-books-by-title\ntitle: \"Sort books by title\"\n\
             context: \"Readers browse by title.\"\nstate:\n  kind: proposed\n"
        ),
    )
    .unwrap();
    let document = compared(&scratch, &git);
    assert_eq!(keys(&document, "added"), [ADDED_KEY, SORT_BY_TITLE_KEY]);
    let added = &document["base"]["added"][1];
    assert_eq!(added["kind"], "decision");
    assert_eq!(added["name"], "Sort books by title");
    assert_eq!(added["label"], Value::Null);
    assert_eq!(added["path"], format!("spec/{SORT_BY_TITLE}"));
    assert_eq!(added["line"], 1);
    let text = stdout(&status(&scratch, &["--base", "develop"], &git));
    assert!(
        text.contains(&format!(
            "added  {SORT_BY_TITLE_KEY}  -  decision  spec/{SORT_BY_TITLE}:1:6  Sort books by title"
        )),
        "{text}"
    );
}

#[test]
fn a_removed_decision_is_listed_with_the_bases_label_and_site() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    fs::remove_file(scratch.file(CHANGE_LOG)).unwrap();
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    let document = compared(&scratch, &git);
    assert_eq!(keys(&document, "removed"), [CHANGE_LOG_KEY]);
    let removed = &document["base"]["removed"][0];
    assert_eq!(removed["label"], "ADR-5");
    assert_eq!(removed["kind"], "decision");
    assert_eq!(removed["name"], "Keep a change log");
    assert_eq!(removed["path"], format!("spec/{CHANGE_LOG}"));
}

#[test]
fn a_decision_whose_only_change_is_a_subject_added_is_listed_as_changed() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.append(
        CHANGE_LOG,
        &format!("  - role: affected\n    node: {IN_ORDER_KEY}\n"),
    );
    let document = compared(&scratch, &git);
    assert_eq!(keys(&document, "changed"), [CHANGE_LOG_KEY]);
    assert_eq!(document["base"]["changed"][0]["label"], "ADR-5");
    assert!(keys(&document, "added").is_empty() && keys(&document, "removed").is_empty());
}

#[test]
fn a_feature_whose_only_change_is_a_realizes_entry_removed_is_listed_as_changed() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.edit(
        ADD_A_BOOK,
        "realizes:\n  - 00000000-0000-4000-8000-00000000003c\n",
        "",
    );
    let document = compared(&scratch, &git);
    assert_eq!(keys(&document, "changed"), [ADD_A_BOOK_KEY]);
    assert_eq!(document["base"]["changed"][0]["label"], "F-1");
}

#[test]
fn a_decision_that_loft_fmt_names_a_removed_node_in_by_its_former_label_is_not_listed() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(STORE_A_SHELF, "  kind: accepted\n", "  kind: retired\n");
    scratch.edit(
        STORE_A_SHELF,
        "  outcome: \"One JSON file per shelf.\"\n",
        "  outcome: \"One JSON file per shelf.\"\n  rationale: \"Shelves moved to a database.\"\n",
    );
    let git = based_on_now(&scratch);
    let feature = fs::read_to_string(scratch.file(ADD_A_BOOK)).unwrap();
    let start = feature
        .find(&format!("  - key: {KEEP_ORDER_KEY}\n"))
        .unwrap();
    let end = feature.find("qualityAttributes:\n").unwrap();
    scratch.edit(ADD_A_BOOK, &feature[start..end], "");
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    for decision in [CHANGE_LOG, STORE_A_SHELF] {
        let written = fs::read_to_string(scratch.file(decision)).unwrap();
        assert!(written.contains("formerLabel: RQ-2"), "{written}");
    }
    let document = compared(&scratch, &git);
    assert!(keys(&document, "changed").is_empty(), "{document}");
    assert_eq!(keys(&document, "removed"), [KEEP_ORDER_KEY, IN_ORDER_KEY]);
}

/// The first of Add a book's `mentions` entries: Remove a book, and the Shelf store, which it
/// also realizes.
const MENTIONS: &str = "mentions:\n  - 00000000-0000-4000-8000-00000000001f\n  - 00000000-0000-4000-8000-00000000003c\n";

#[test]
fn a_node_whose_only_change_is_a_mentions_entry_removed_is_listed_as_changed() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    // The Shelf store stays among the Components Add a book realizes.
    scratch.edit(
        ADD_A_BOOK,
        MENTIONS,
        "mentions:\n  - 00000000-0000-4000-8000-00000000001f\n",
    );
    let document = compared(&scratch, &git);
    assert_eq!(keys(&document, "changed"), [ADD_A_BOOK_KEY]);
}

#[test]
fn a_decision_whose_only_change_is_a_mentions_entry_added_is_listed_as_changed() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.append(CHANGE_LOG, &format!("mentions:\n  - {ADD_A_BOOK_KEY}\n"));
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    let document = compared(&scratch, &git);
    assert_eq!(keys(&document, "changed"), [CHANGE_LOG_KEY]);
}

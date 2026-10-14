//! `loft find` (the `loft find` Feature): the nodes and decisions a query identifies by label,
//! slug, key or name, each on one line with its key, label, kind, site and name.

use std::process::Output;

use crate::common::{Scratch, stderr, stdout};
use crate::schema;
use crate::snapshot::snapshot;
use serde_json::Value;

const ADD_A_BOOK: &str = "00000000-0000-4000-8000-00000000001e";
const FEATURE: &str = "products/reading-list/shelf/add-a-book.yaml";

/// The fields of each line the run printed.
fn matches(output: &Output) -> Vec<Vec<String>> {
    stdout(output)
        .lines()
        .map(|line| line.split("  ").map(str::to_owned).collect())
        .collect()
}

fn json_of(output: &Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/find.json"))
}

#[test]
fn a_label_finds_its_element_on_one_line_with_key_label_kind_site_and_name() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["find", "F-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let found = matches(&output);
    assert_eq!(found.len(), 1, "{}", stdout(&output));
    let [key, label, kind, site, name] = found[0].as_slice() else {
        panic!("five fields: {:?}", found[0]);
    };
    assert_eq!(
        [key.as_str(), label, kind, name],
        [ADD_A_BOOK, "F-1", "feature", "Add a book"]
    );
    assert!(site.starts_with(&format!("spec/{FEATURE}:1:")), "{site}");
}

#[test]
fn a_slug_and_a_key_find_their_element() {
    let scratch = Scratch::with_reading_list();
    for query in ["add-a-book", ADD_A_BOOK] {
        let found = matches(&scratch.loft(&["find", query]));
        assert_eq!(found.len(), 1, "{query}");
        assert_eq!(found[0][0], ADD_A_BOOK, "{query}");
    }
}

#[test]
fn a_name_containing_the_query_in_any_case_is_found_with_decisions_by_title() {
    let scratch = Scratch::with_reading_list();
    let found = matches(&scratch.loft(&["find", "SHELF STORE"]));
    let names: Vec<&str> = found.iter().map(|fields| fields[4].as_str()).collect();
    assert!(names.contains(&"Shelf store"), "{names:?}");
    let found = matches(&scratch.loft(&["find", "a shelf in one file"]));
    assert_eq!(found[0][2], "decision");
    assert_eq!(found[0][4], "Store a shelf in one file");
}

#[test]
fn a_requirement_is_found_by_its_statement_as_one_sentence() {
    let scratch = Scratch::with_reading_list();
    let found = matches(&scratch.loft(&["find", "the shelf shall record"]));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0][2], "requirement");
    assert_eq!(
        found[0][4],
        "When a reader adds a book, the shelf shall record its title and author."
    );
}

#[test]
fn an_element_without_a_label_is_printed_with_a_dash() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(
        FEATURE,
        "    criteria:\n",
        "    criteria:\n      - name: \"A shelved book keeps its cover.\"\n",
    );
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    let found = matches(&scratch.loft(&["find", "keeps its cover"]));
    assert_eq!(found[0][1], "-");
    let output = scratch.loft(&["find", "keeps its cover", "--format", "json"]);
    assert_eq!(json_of(&output)["matches"][0]["label"], Value::Null);
}

#[test]
fn a_query_that_matches_nothing_says_so_and_exits_1() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["find", "no such thing"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("nothing matches"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_json_document_lists_each_match_with_its_fields_and_the_outcome() {
    let scratch = Scratch::with_reading_list();
    let output = scratch.loft(&["find", "F-1", "--format", "json"]);
    let document = json_of(&output);
    assert_eq!(document["outcome"], "found");
    let found = &document["matches"][0];
    assert_eq!(found["key"], ADD_A_BOOK);
    assert_eq!(found["label"], "F-1");
    assert_eq!(found["kind"], "feature");
    assert_eq!(found["path"], format!("spec/{FEATURE}"));
    assert_eq!(found["line"], 1);
    assert!(found["column"].is_u64());
    assert_eq!(found["name"], "Add a book");
    let none = scratch.loft(&["find", "no such thing", "--format", "json"]);
    assert_eq!(none.status.code(), Some(1));
    assert_eq!(json_of(&none)["outcome"], "none");
}

#[test]
fn find_writes_no_file() {
    let scratch = Scratch::with_reading_list();
    let before = snapshot(&scratch.0);
    scratch.loft(&["find", "shelf"]);
    assert_eq!(snapshot(&scratch.0), before);
}

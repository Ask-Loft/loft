//! Interfaces through `loft`: found by `loft find`, listed by `loft status` against a base,
//! reviewed as the scope of their Requirements and rendered, over the Reading List with an
//! Interface its Shelf store provides.

use std::fs;

use crate::base::based_on_now;
use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::schema;
use serde_json::Value;
use software_plant::Clock;

const STORE: &str = "components/shelf-store.yaml";
const SCOPE: &str = "scopedTo:\n  - 00000000-0000-4000-8000-00000000000a\n";
const LOOKUP: &str = "00000000-0000-4000-8000-0000000000b0";
const PROMISE: &str = "00000000-0000-4000-8000-0000000000b1";
const ANSWER: &str = "00000000-0000-4000-8000-0000000000b2";
const FILE: &str = "interfaces/shelf-lookup.yaml";

/// Writes the Interface "Shelf lookup" and has the Shelf store provide it.
fn add_lookup(scratch: &Scratch) {
    let path = scratch.file(FILE);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            "key: {LOOKUP}\nslug: shelf-lookup\nname: \"Shelf lookup\"\nbody: \"What a reader \
             of a shelf may rely on.\"\nrequirements:\n  - key: {PROMISE}\n    statement:\n      \
             form: event_driven\n      when: \"a reader asks for a book by its title\"\n      \
             system: \"the shelf store\"\n      shall: \"return the book.\"\n    criteria:\n      \
             - key: {ANSWER}\n        name: \"The returned book has the title asked for.\"\n"
        ),
    )
    .unwrap();
    scratch.edit(STORE, SCOPE, &format!("{SCOPE}provides:\n  - {LOOKUP}\n"));
}

fn with_lookup() -> Scratch {
    let scratch = Scratch::with_reading_list();
    add_lookup(&scratch);
    let output = scratch.loft(&["fmt", "--check"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch
}

#[test]
fn an_interface_is_found_by_its_slug_and_its_name_with_the_kind_interface() {
    let scratch = with_lookup();
    for query in ["shelf-lookup", "SHELF LOOKUP", LOOKUP] {
        let output = scratch.loft(&["find", query, "--format", "json"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{query}: {}",
            stderr(&output)
        );
        let document =
            schema::conforming(&output.stdout, include_str!("../../../schemas/find.json"));
        let found = &document["matches"][0];
        assert_eq!(found["key"], LOOKUP, "{query}");
        assert_eq!(found["kind"], "interface", "{query}");
        assert_eq!(found["path"], format!("spec/{FILE}"), "{query}");
    }
}

#[test]
fn an_added_interface_and_what_it_holds_are_listed_added_against_a_base() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    add_lookup(&scratch);
    let output = scratch.loft_with(
        &["status", "--base", "develop", "--format", "json"],
        Some(&git),
        &Clock::fixed(EPOCH),
    );
    let document = schema::conforming(&output.stdout, include_str!("../../../schemas/status.json"));
    let added: Vec<(&str, &str)> = document["base"]["added"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| {
            (
                node["key"].as_str().unwrap(),
                node["kind"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        added,
        [
            (LOOKUP, "interface"),
            (PROMISE, "requirement"),
            (ANSWER, "criterion")
        ]
    );
    let changed: Vec<&str> = document["base"]["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["kind"].as_str().unwrap())
        .collect();
    assert_eq!(changed, ["component"]);
}

#[test]
fn an_interface_is_the_scope_of_a_review_unit_of_kind_interface() {
    let scratch = with_lookup();
    let output = scratch.loft(&["review", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = schema::conforming(&output.stdout, include_str!("../../../schemas/review.json"));
    let unit = document["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["scope"]["node"]["key"] == LOOKUP)
        .unwrap_or_else(|| panic!("no unit of the Interface in {document}"));
    assert_eq!(unit["scope"]["node"]["kind"], "interface");
    let assertions: Vec<&Value> = unit["assertions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|assertion| &assertion["key"])
        .collect();
    assert_eq!(assertions, [PROMISE, ANSWER]);
}

#[test]
fn a_render_writes_the_interfaces_section_and_the_provides_line() {
    let scratch = with_lookup();
    let output = scratch.loft(&["render"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let markdown = fs::read_to_string(scratch.0.join("docs/spec.md")).unwrap();
    assert!(markdown.contains("\n## Interfaces\n"), "{markdown}");
    assert!(markdown.contains(" · Shelf lookup\n"), "{markdown}");
    assert!(markdown.contains("\nProvides: "), "{markdown}");
    assert_eq!(stdout(&output), "docs/spec.md\ndocs/spec.json\n");
}

//! What a `spec-change` view records of a changed Section: a box of its own beneath the box of
//! the node that holds it, which is drawn only for context, and a diff entry under the Section's
//! own key.

use serde_json::Value;

use crate::yard::Yard;
use crate::yard_view_constraints::{ADD_A_BOOK, assert_boxed_beneath, diagram};
use crate::yard_view_reports::node;

/// The Component that holds a Section in [`with_sections`].
const SHELF_STORE: &str = "components/shelf-store.yaml";

/// The last criterion of Add a book's second Requirement, after which its Section is written.
const IN_ORDER: &str = "        name: \"Books list in the order they were added.\"\n";

/// The body of the Feature's Section.
const TITLES: &str = "Readers remember a title better than an author.";

/// The body of the Section nested in the Feature's Section.
const SPINES: &str = "A title is printed on the spine.";

/// The body of the Requirement's Section.
const ORDER: &str = "Readers find the book they added last at the end.";

/// The body of the Component's Section.
const FILES: &str = "Each shelf is one file.";

/// The Shelf store's Section, as [`with_sections`] writes it.
const LAYOUT: &str = "sections:\n  - key: 00000000-0000-4000-8000-0000000000b4\n    \
     name: \"Layout\"\n    body: \"Each shelf is one file.\"\n";

/// The statement of Add a book's second Requirement, which holds a Section.
const KEEP_ORDER: &str = "The shelf shall keep books in the order they were added.";

/// The Reading List with a Section in Add a book holding a Section of its own, a Section in its
/// second Requirement and a Section in the Shelf store, each in the base as in the tree.
fn with_sections() -> Yard {
    let mut yard = Yard::with_spec();
    yard.scratch.append(
        ADD_A_BOOK,
        &format!(
            "sections:\n  - key: 00000000-0000-4000-8000-0000000000b1\n    \
             name: \"Why titles\"\n    body: \"{TITLES}\"\n    sections:\n      \
             - key: 00000000-0000-4000-8000-0000000000b2\n        name: \"Spines\"\n        \
             body: \"{SPINES}\"\n"
        ),
    );
    yard.scratch.edit(
        ADD_A_BOOK,
        IN_ORDER,
        &format!(
            "{IN_ORDER}    sections:\n      - key: 00000000-0000-4000-8000-0000000000b3\n        \
             name: \"Why order\"\n        body: \"{ORDER}\"\n"
        ),
    );
    yard.scratch.append(SHELF_STORE, LAYOUT);
    yard.rebased();
    yard
}

#[test]
fn a_spec_change_view_draws_a_changed_section_as_a_box_of_its_own_and_its_feature_only_for_context()
{
    let yard = with_sections();
    yard.scratch
        .edit(ADD_A_BOOK, TITLES, "Readers remember a title best.");
    let graph = diagram(&yard);
    assert_boxed_beneath(&graph, "Why titles", "section", "changed", "Add a book");
    let section = node(&graph, "Why titles");
    let entries = graph["diff"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{graph}");
    assert_eq!(
        entries[0]["key"], section["key"],
        "choosing the Section's box finds its own entry: {graph}"
    );
}

#[test]
fn a_spec_change_view_draws_the_changed_sections_of_a_requirement_a_component_and_a_section_beside_their_boxes()
 {
    let yard = with_sections();
    yard.scratch
        .edit(ADD_A_BOOK, ORDER, "The book added last is listed last.");
    yard.scratch
        .edit(ADD_A_BOOK, SPINES, "A title is printed on the cover.");
    yard.scratch
        .edit(SHELF_STORE, FILES, "Each shelf is one directory.");
    let graph = diagram(&yard);
    assert_boxed_beneath(&graph, "Why order", "section", "changed", KEEP_ORDER);
    assert_boxed_beneath(&graph, "Layout", "section", "changed", "Shelf store");
    assert_boxed_beneath(&graph, "Spines", "section", "changed", "Why titles");
    let marked: Vec<&Value> = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|box_| !box_["change"].is_null())
        .map(|box_| &box_["kind"])
        .collect();
    assert_eq!(
        marked, ["section"; 3],
        "only the Sections are marked: {graph}"
    );
}

#[test]
fn a_spec_change_view_draws_an_added_and_a_removed_section_as_boxes_of_their_own() {
    let yard = with_sections();
    yard.scratch.edit(SHELF_STORE, LAYOUT, "");
    yard.scratch.edit(
        ADD_A_BOOK,
        "    sections:\n      - key: 00000000-0000-4000-8000-0000000000b2\n",
        "    sections:\n      - key: 00000000-0000-4000-8000-0000000000b5\n        \
         name: \"Covers\"\n        body: \"A cover shows the title.\"\n      \
         - key: 00000000-0000-4000-8000-0000000000b2\n",
    );
    let graph = diagram(&yard);
    assert_boxed_beneath(&graph, "Covers", "section", "added", "Why titles");
    assert_boxed_beneath(&graph, "Layout", "section", "removed", "Shelf store");
}

//! What a `spec-change` view records of a changed Section: a box of its own beneath the box of
//! the node that holds it, which is drawn only for context, and a diff entry under the Section's
//! own key.

use serde_json::Value;

use crate::yard::Yard;
use crate::yard_view_reports::{node, parent, report};

/// The Feature that the Sections of [`with_sections`] are written into.
const ADD_A_BOOK: &str = "products/reading-list/shelf/add-a-book.yaml";

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
    yard.scratch.append(
        SHELF_STORE,
        &format!(
            "sections:\n  - key: 00000000-0000-4000-8000-0000000000b4\n    \
             name: \"Layout\"\n    body: \"{FILES}\"\n"
        ),
    );
    yard.rebased();
    yard
}

/// The diagram of a `spec-change` view of the tree as it now stands.
fn diagram(yard: &Yard) -> Value {
    report(yard, "spec-change")["graph"].clone()
}

/// Asserts that `graph` draws the Section named `name` as a box of its own marked `change`,
/// beneath the box of `kind` that holds it, which is drawn only for context; gives that box.
fn assert_section_beneath<'a>(graph: &'a Value, name: &str, change: &str, kind: &str) -> &'a Value {
    let section = node(graph, name);
    assert_eq!(section["kind"], "section", "{section}");
    assert_eq!(section["change"], change, "{section}");
    let above = parent(graph, section);
    assert_eq!(above["kind"], kind, "{graph}");
    assert_eq!(
        above["change"],
        Value::Null,
        "its own content did not change: {above}"
    );
    above
}

#[test]
fn a_spec_change_view_draws_a_changed_section_as_a_box_of_its_own_and_its_feature_only_for_context()
{
    let yard = with_sections();
    yard.scratch
        .edit(ADD_A_BOOK, TITLES, "Readers remember a title best.");
    let graph = diagram(&yard);
    let feature = assert_section_beneath(&graph, "Why titles", "changed", "feature");
    assert_eq!(feature["name"], "Add a book");
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
    assert_section_beneath(&graph, "Why order", "changed", "requirement");
    assert_section_beneath(&graph, "Layout", "changed", "component");
    let holder = assert_section_beneath(&graph, "Spines", "changed", "section");
    assert_eq!(holder["name"], "Why titles");
    for kind in ["feature", "requirement", "component", "section"] {
        let marked = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|box_| box_["kind"] == kind && !box_["change"].is_null())
            .count();
        let expected = usize::from(kind == "section") * 3;
        assert_eq!(marked, expected, "only the Sections are marked: {graph}");
    }
}

#[test]
fn a_spec_change_view_draws_an_added_and_a_removed_section_as_boxes_of_their_own() {
    let yard = with_sections();
    let section = format!(
        "sections:\n  - key: 00000000-0000-4000-8000-0000000000b4\n    \
         name: \"Layout\"\n    body: \"{FILES}\"\n"
    );
    yard.scratch.edit(SHELF_STORE, &section, "");
    yard.scratch.edit(
        ADD_A_BOOK,
        "    sections:\n      - key: 00000000-0000-4000-8000-0000000000b2\n",
        "    sections:\n      - key: 00000000-0000-4000-8000-0000000000b5\n        \
         name: \"Covers\"\n        body: \"A cover shows the title.\"\n      \
         - key: 00000000-0000-4000-8000-0000000000b2\n",
    );
    let graph = diagram(&yard);
    assert_section_beneath(&graph, "Covers", "added", "section");
    assert_section_beneath(&graph, "Layout", "removed", "component");
}

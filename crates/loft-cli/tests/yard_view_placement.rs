//! Where a `spec-change` view draws each node and edge: as the tree holds it, and a removed one
//! as the base held it. A `realizes` edge the branch removed between two nodes the tree still holds
//! is not drawn, and the Feature that lost it is drawn as changed.

use std::fs;

use serde_json::Value;

use crate::yard::Yard;
use crate::yard_view_constraints::{ADD_A_BOOK, diagram};
use crate::yard_view_reports::node;

/// The Shelf store's key, which Add a book and Remove a book realize.
const SHELF_STORE_KEY: &str = "00000000-0000-4000-8000-00000000003c";
/// Add a book's key.
const ADD_A_BOOK_KEY: &str = "00000000-0000-4000-8000-00000000001e";
/// The Feature Share a list, which realizes the Link service, and the Link service's key.
const SHARE_A_LIST: &str = "products/reading-list/sharing/share-a-list.yaml";
const SHARE_A_LIST_KEY: &str = "00000000-0000-4000-8000-000000000020";
const LINK_SERVICE: &str = "components/link-service.yaml";
const LINK_SERVICE_KEY: &str = "00000000-0000-4000-8000-000000000041";

/// The `realizes` edges of `graph`, as (from, to).
fn realizes(graph: &Value) -> Vec<(String, String)> {
    graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| edge["kind"] == "realizes")
        .map(|edge| {
            let end = |field: &str| edge[field].as_str().unwrap().to_owned();
            (end("from"), end("to"))
        })
        .collect()
}

#[test]
fn a_spec_change_view_draws_no_realizes_edge_the_branch_removed_between_nodes_the_tree_holds() {
    let yard = Yard::with_spec();
    yard.scratch.edit(
        ADD_A_BOOK,
        &format!("realizes:\n  - {SHELF_STORE_KEY}\n"),
        "",
    );
    // The Book index changes too, so that the Shelf store's box is drawn and an edge to it could be.
    yard.scratch.edit(
        "components/shelf-store.yaml",
        "    name: \"Book index\"\n",
        "    name: \"Book catalog\"\n",
    );
    let graph = diagram(&yard);
    assert_eq!(node(&graph, "Shelf store")["key"], SHELF_STORE_KEY);
    let feature = node(&graph, "Add a book");
    assert_eq!(feature["change"], "changed", "{graph}");
    assert!(
        !realizes(&graph).contains(&(ADD_A_BOOK_KEY.to_owned(), SHELF_STORE_KEY.to_owned())),
        "{graph}"
    );
}

#[test]
fn a_spec_change_view_draws_the_realizes_edge_of_a_removed_feature_as_the_base_held_it() {
    let yard = Yard::with_spec();
    fs::remove_file(yard.scratch.file(SHARE_A_LIST)).unwrap();
    yard.scratch.edit(
        "products/reading-list/product.yaml",
        &format!("    features:\n      - {SHARE_A_LIST_KEY}\n"),
        "",
    );
    let formatted = yard.scratch.loft(&["fmt"]);
    assert!(formatted.status.success(), "{formatted:?}");
    yard.scratch.edit(
        LINK_SERVICE,
        "name: \"Link service\"",
        "name: \"Share service\"",
    );
    let graph = diagram(&yard);
    assert_eq!(node(&graph, "Share a list")["change"], "removed", "{graph}");
    assert_eq!(
        realizes(&graph),
        [(SHARE_A_LIST_KEY.to_owned(), LINK_SERVICE_KEY.to_owned())],
        "{graph}"
    );
}

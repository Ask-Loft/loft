//! The Markdown projection and the canonical JSON export (F-24, F-25).

mod common;

use common::small_document;
use loftspec::canonical::canonical_bytes;
use loftspec::codec;
use loftspec::hash::{hash_document, hash_indexed_with_views};
use loftspec::render::{Projections, render};
use serde_json::Value;

fn projections() -> Projections {
    render(&small_document()).unwrap()
}

fn markdown() -> String {
    projections().markdown
}

fn export() -> Value {
    serde_json::from_slice(&projections().json).unwrap()
}

fn state_root() -> String {
    hash_document(&small_document())
        .unwrap()
        .roots
        .state
        .to_string()
}

#[test]
fn capabilities_render_as_headings_of_levels_3_to_6() {
    let markdown = markdown();
    for line in [
        "### P-1 · Example Product\n",
        "#### FG-1 · Identity\n",
        "##### F-1 · Keys\n",
        "###### RQ-2 · The writer shall write a key on every node.\n",
    ] {
        assert!(markdown.contains(line), "{line}");
    }
}

#[test]
fn a_criterion_is_a_bold_paragraph_and_an_unlabelled_node_shows_its_key() {
    assert!(markdown().contains(
        "\n**00000000-0000-7000-8000-000000000005 · Acceptance Criterion · Every node record \
         carries a `key` field.**\n"
    ));
}

#[test]
fn components_decisions_tags_and_realizes_edges_follow_the_products() {
    let markdown = markdown();
    let at = |heading: &str| {
        markdown
            .find(heading)
            .unwrap_or_else(|| panic!("{heading}"))
    };
    let order = [
        at("## Capability Model\n"),
        at("## Component Model\n"),
        at("## Architecture Decisions\n"),
        at("## Domain Vocabulary\n"),
        at("## Realization Links\n"),
    ];
    assert!(order.is_sorted(), "{order:?}");
    assert!(markdown.contains("| F-1 · Keys | CMP-1 · Document model |\n"));
}

#[test]
fn every_node_of_the_document_appears() {
    let markdown = markdown();
    for label in [
        "P-1", "FG-1", "F-1", "RQ-2", "CMP-1", "CON-1", "CON-2", "QA-1", "QA-2", "ADR-1",
    ] {
        assert!(markdown.contains(&format!("{label} · ")), "{label}");
    }
}

#[test]
fn a_nodes_tags_render_on_one_line_after_its_body() {
    assert!(markdown().contains("\n\nThe example.\n\nTags: `portable`\n\n"));
}

#[test]
fn the_title_is_the_first_products_name() {
    assert!(markdown().starts_with("# Example Product\n\n"));
}

#[test]
fn root_components_render_in_the_order_the_reader_takes_them() {
    let mut document = small_document();
    document
        .components
        .push(common::root(common::component(common::key(50), "Alpha")));
    let read = codec::read(&codec::write(&document)).unwrap();
    let markdown = render(&read).unwrap().markdown;
    let at = |name: &str| markdown.find(&format!(" · {name}\n")).unwrap();
    assert!(at("Alpha") < at("Document model"), "{markdown}");
}

#[test]
fn two_renders_are_byte_identical() {
    assert_eq!(projections(), projections());
}

#[test]
fn the_markdown_names_its_state_root_and_no_other_digest() {
    let markdown = markdown();
    let root = state_root();
    assert!(markdown.contains(&format!("> loftspec/1 · state root `{root}`\n")));
    let digests = markdown
        .split(|c: char| !c.is_ascii_hexdigit())
        .filter(|word| word.len() == 64)
        .collect::<Vec<_>>();
    assert_eq!(digests, [root.as_str()]);
}

#[test]
fn the_export_is_canonical_json() {
    let bytes = projections().json;
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(canonical_bytes(&value), bytes);
}

#[test]
fn the_export_holds_every_nodes_view_key_and_label() {
    let export = export();
    let (_, views) = hash_indexed_with_views(&small_document().index().unwrap());
    let nodes = export["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), views.nodes.len());
    let feature = nodes.iter().find(|node| node["label"] == "F-1").unwrap();
    assert_eq!(feature["key"], "00000000-0000-7000-8000-000000000003");
    assert_eq!(feature["kind"], "feature");
    let id = loftspec::ids::FeatureId::new(common::key(3)).into();
    assert_eq!(feature["view"], views.nodes[&id]);
    let criterion = nodes
        .iter()
        .find(|node| node["kind"] == "criterion")
        .unwrap();
    assert_eq!(criterion["label"], Value::Null);
}

#[test]
fn the_export_names_its_state_root() {
    assert_eq!(export()["stateRoot"], state_root());
}

#[test]
fn the_export_holds_edges_decisions_and_registries() {
    let export = export();
    let edges = export["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 5);
    assert!(
        edges
            .iter()
            .any(|edge| edge["type"] == "subject" && edge["role"] == "outcome")
    );
    assert_eq!(export["decisions"][0]["label"], "ADR-1");
    assert_eq!(export["tags"].as_array().unwrap().len(), 2);
    assert_eq!(export["categories"][0]["name"], "runtime.language");
}

#[test]
fn the_export_layout_is_the_published_one() {
    let published = include_bytes!("../../../testdata/golden/small-document.export.json");
    assert_eq!(projections().json, published);
}

#[test]
fn a_code_span_in_a_name_renders_as_written() {
    let mut document = small_document();
    document.products[0].feature_groups[0].name =
        "The `event_driven` form and snake_case [notes]".to_owned();
    let markdown = render(&document).unwrap().markdown;
    assert!(
        markdown.contains("The `event_driven` form and snake\\_case \\[notes\\]"),
        "{markdown}"
    );
}

#[test]
fn a_bar_in_a_code_span_is_still_escaped() {
    let mut document = small_document();
    document.products[0].feature_groups[0].name = "The `a|b` form".to_owned();
    let markdown = render(&document).unwrap().markdown;
    assert!(markdown.contains("The `a\\|b` form"), "{markdown}");
}

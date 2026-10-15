//! Extension data: a node record's `ext`, read and written in the text form, kept out of every
//! content hash and every lint rule, held by the state root through the extensions root, and
//! carried by the JSON export.

use std::collections::BTreeMap;

use loftspec::canonical::{Domain, digest};
use loftspec::check::check;
use loftspec::codec::{self, Files, SpecPath};
use loftspec::document::Document;
use loftspec::ext::{self, Ext, ExtValue};
use loftspec::hash::{ContentHash, hash_document, hash_indexed_with_views};
use loftspec::merge::merge_file;
use loftspec::names::Namespace;
use loftspec::render::render;
use loftspec::tree::{Component, Details, Feature, Section};
use serde_json::json;

use crate::common::full::full_nested_document;
use crate::common::small_document;

/// Extension data holding every kind of value.
fn sample() -> Ext {
    let metadata = ExtValue::List(vec![ExtValue::Map(BTreeMap::from([
        ("label".to_owned(), ExtValue::Text("ID".to_owned())),
        ("value".to_owned(), ExtValue::Text("AUTH-1".to_owned())),
    ]))]);
    let openspec = ExtValue::Map(BTreeMap::from([
        ("name".to_owned(), ExtValue::Text("Sign in".to_owned())),
        ("notes".to_owned(), ExtValue::Text("Two\nlines.".to_owned())),
        ("order".to_owned(), ExtValue::Integer(-3)),
        ("draft".to_owned(), ExtValue::Boolean(true)),
        (
            "tags".to_owned(),
            ExtValue::List(vec![
                ExtValue::Text("zeta".to_owned()),
                ExtValue::Text("alpha".to_owned()),
            ]),
        ),
        ("metadata".to_owned(), metadata),
    ]));
    Ext::from([(Namespace::parse("openspec").unwrap(), openspec)])
}

fn sections(list: &mut [Section], set: &mut impl FnMut(&mut Ext)) {
    for section in list {
        set(&mut section.ext);
        sections(&mut section.sections, set);
    }
}

fn details(details: &mut Details, set: &mut impl FnMut(&mut Ext)) {
    set(&mut details.ext);
    sections(&mut details.sections, set);
}

fn component(component: &mut Component, set: &mut impl FnMut(&mut Ext)) {
    details(&mut component.details, set);
    for constraint in &mut component.constraints {
        details(&mut constraint.details, set);
    }
    for quality in &mut component.quality_attributes {
        details(&mut quality.details, set);
    }
    for child in &mut component.components {
        self::component(child, set);
    }
}

fn feature(feature: &mut Feature, set: &mut impl FnMut(&mut Ext)) {
    details(&mut feature.details, set);
    for quality in &mut feature.quality_attributes {
        details(&mut quality.details, set);
    }
    for requirement in &mut feature.requirements {
        details(&mut requirement.details, set);
        for criterion in &mut requirement.criteria {
            details(&mut criterion.details, set);
        }
    }
}

/// Applies `set` to the extension data of every node of `document`.
fn every_node(document: &mut Document, set: &mut impl FnMut(&mut Ext)) {
    for product in &mut document.products {
        details(&mut product.details, set);
        for constraint in &mut product.constraints {
            details(&mut constraint.details, set);
        }
        for group in &mut product.feature_groups {
            details(&mut group.details, set);
            for constraint in &mut group.constraints {
                details(&mut constraint.details, set);
            }
            for feature in &mut group.features {
                self::feature(feature, set);
            }
        }
    }
    for root in &mut document.components {
        component(&mut root.component, set);
    }
}

/// The small document's first Requirement, to change.
fn first_requirement(document: &mut Document) -> &mut loftspec::tree::Requirement {
    &mut document.products[0].feature_groups[0].features[0].requirements[0]
}

/// The path and text of the small document's file that holds its first Feature.
fn feature_file(files: &Files) -> (SpecPath, String) {
    let document = small_document();
    let feature = &document.products[0].feature_groups[0].features[0];
    let key = feature.id.unwrap().key().to_string();
    files
        .iter()
        .map(|(path, bytes)| (path.clone(), String::from_utf8(bytes.clone()).unwrap()))
        .find(|(_, text)| text.starts_with(&format!("key: {key}")))
        .unwrap()
}

/// The read errors of the small document with `ext` appended to its first Feature's record.
fn errors_with(ext: &str) -> String {
    let mut files = codec::write(&small_document());
    let (path, text) = feature_file(&files);
    files.insert(path, format!("{text}ext:\n{ext}").into_bytes());
    let errors = codec::read(&files).expect_err("the ext does not read");
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_node_kind_carries_ext_through_a_write_and_a_read() {
    let mut document = full_nested_document();
    every_node(&mut document, &mut |ext| *ext = sample());
    let files = codec::write(&document);
    let exts = |document: &Document| {
        let mut exts = Vec::new();
        document.walk(&mut |node| exts.push((node.id(), node.ext().clone())));
        exts
    };
    let read = codec::read(&files).unwrap();
    assert_eq!(exts(&read), exts(&document));
    assert!(exts(&read).iter().all(|(_, ext)| *ext == sample()));
    assert_eq!(codec::write(&codec::read(&files).unwrap()), files);
}

#[test]
fn ext_is_written_after_mentions_with_its_keys_sorted_and_its_lists_in_order() {
    let mut document = small_document();
    first_requirement(&mut document).details.ext = sample();
    let (_, text) = feature_file(&codec::write(&document));
    let expected = "    ext:\n      openspec:\n        draft: true\n        metadata:\n          - label: \"ID\"\n            value: \"AUTH-1\"\n        name: \"Sign in\"\n        notes: |-\n          Two\n          lines.\n        order: -3\n        tags:\n          - \"zeta\"\n          - \"alpha\"\n";
    assert!(text.contains(expected), "{text}");
}

#[test]
fn a_decision_record_holding_ext_is_an_error() {
    let mut files = codec::write(&small_document());
    let path = files
        .keys()
        .find(|path| path.as_str().starts_with("decisions/"))
        .cloned()
        .unwrap();
    let mut text = String::from_utf8(files[&path].clone()).unwrap();
    text.push_str("ext:\n  openspec:\n    name: \"A\"\n");
    files.insert(path, text.into_bytes());
    let errors = codec::read(&files).expect_err("a decision holds no ext");
    assert!(errors.iter().any(|error| error.to_string().contains("ext")));
}

#[test]
fn a_namespace_outside_its_syntax_is_an_error_at_the_namespace() {
    let errors = errors_with("  OpenSpec:\n    name: \"A\"\n");
    assert!(
        errors.contains("OpenSpec") && errors.contains("namespace"),
        "{errors}"
    );
}

#[test]
fn a_plain_scalar_other_than_an_integer_or_a_boolean_is_an_error() {
    for value in ["1.5", "1e3", "null", "~", "word", "007", "-0", "True"] {
        let errors = errors_with(&format!("  openspec:\n    value: {value}\n"));
        assert!(errors.contains(value), "{value}: {errors}");
    }
}

#[test]
fn an_integer_out_of_range_is_an_error_and_the_bounds_read() {
    for outside in [
        "9007199254740992",
        "-9007199254740992",
        "-9223372036854775808",
    ] {
        let errors = errors_with(&format!("  openspec:\n    value: {outside}\n"));
        assert!(errors.contains(outside), "{errors}");
    }
    for bound in ["9007199254740991", "-9007199254740991"] {
        let mut files = codec::write(&small_document());
        let (path, text) = feature_file(&files);
        let ext = format!("ext:\n  openspec:\n    value: {bound}\n");
        files.insert(path, format!("{text}{ext}").into_bytes());
        assert!(codec::read(&files).is_ok(), "{bound}");
    }
}

#[test]
fn a_field_with_no_value_is_an_error() {
    let errors = errors_with("  openspec:\n    value:\n");
    assert!(errors.contains("a value"), "{errors}");
}

#[test]
fn ext_leaves_content_hashes_and_the_content_root_unchanged() {
    let plain = small_document();
    let mut extended = plain.clone();
    every_node(&mut extended, &mut |ext| *ext = sample());
    let (before, after) = (
        hash_document(&plain).unwrap(),
        hash_document(&extended).unwrap(),
    );
    assert_eq!(before.nodes, after.nodes);
    assert_eq!(before.roots.content, after.roots.content);
    let feature = &extended.products[0].feature_groups[0].features[0];
    let original = &plain.products[0].feature_groups[0].features[0];
    assert_eq!(feature.content_hash(), original.content_hash());
}

#[test]
fn a_document_without_ext_has_no_extensions_root() {
    let hashes = hash_document(&small_document()).unwrap();
    assert_eq!(hashes.roots.extensions, None);
}

#[test]
fn changing_ext_changes_the_extensions_root_and_the_state_root() {
    let plain = small_document();
    let mut first = plain.clone();
    first_requirement(&mut first).details.ext = sample();
    let mut second = first.clone();
    let namespace = Namespace::parse("openspec").unwrap();
    second.products[0].feature_groups[0].features[0].requirements[0]
        .details
        .ext
        .insert(namespace, ExtValue::Text("other".to_owned()));
    let roots = |document: &Document| hash_document(document).unwrap().roots;
    assert!(roots(&first).extensions.is_some());
    assert_ne!(roots(&first).extensions, roots(&second).extensions);
    assert_ne!(roots(&plain).state, roots(&first).state);
    assert_ne!(roots(&first).state, roots(&second).state);
}

#[test]
fn the_export_holds_ext_beside_the_view_of_a_node_that_carries_it() {
    let mut document = small_document();
    first_requirement(&mut document).details.ext = sample();
    let export: serde_json::Value =
        serde_json::from_slice(&render(&document).unwrap().json).unwrap();
    let nodes = export["nodes"].as_array().unwrap();
    let carrying: Vec<_> = nodes
        .iter()
        .filter(|node| node.get("ext").is_some())
        .collect();
    assert_eq!(carrying.len(), 1);
    assert_eq!(carrying[0]["ext"]["openspec"]["order"], -3);
    let plain: serde_json::Value =
        serde_json::from_slice(&render(&small_document()).unwrap().json).unwrap();
    assert!(
        plain["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|node| node.get("ext").is_none())
    );
}

#[test]
fn ext_changes_no_lint_finding_and_no_line_of_the_markdown_projection() {
    let plain = small_document();
    let mut extended = plain.clone();
    let weak = ExtValue::Text("It should be fast and user-friendly, etc.".to_owned());
    every_node(&mut extended, &mut |ext| {
        ext.insert(Namespace::parse("notes").unwrap(), weak.clone());
    });
    let findings = |document: &Document| {
        check(&codec::write(document))
            .findings()
            .iter()
            .map(|finding| (finding.rule, finding.message.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(findings(&plain), findings(&extended));
    let markdown = |document: &Document| render(document).unwrap().markdown;
    let lines = |text: String| {
        text.lines()
            .filter(|line| !line.contains("state root"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(lines(markdown(&plain)), lines(markdown(&extended)));
}

#[test]
fn a_merge_takes_one_sides_ext_and_records_a_conflict_when_both_differ() {
    let base = codec::write(&small_document());
    let (path, _) = feature_file(&base);
    let with = |value: &str| {
        let mut document = small_document();
        let namespace = Namespace::parse("openspec").unwrap();
        first_requirement(&mut document)
            .details
            .ext
            .insert(namespace, ExtValue::Text(value.to_owned()));
        codec::write(&document)[&path].clone()
    };
    let (ours, theirs) = (with("ours"), with("theirs"));
    let merged = merge_file(&path, &base[&path], &ours, &base[&path]).unwrap();
    assert_eq!(merged.bytes, ours);
    let disputed = merge_file(&path, &base[&path], &ours, &theirs).unwrap();
    assert_eq!(disputed.conflicts, 1);
    let text = String::from_utf8(disputed.bytes).unwrap();
    assert!(
        text.contains("field: \"ext\"") || text.contains("field: ext"),
        "{text}"
    );
}

#[test]
fn a_merge_keeps_an_ext_every_side_shares_without_a_conflict() {
    let with = |requirement_body: Option<&str>, feature_body: &str| {
        let mut document = small_document();
        document.products[0].feature_groups[0].features[0]
            .details
            .body = Some(feature_body.to_owned());
        let requirement = first_requirement(&mut document);
        requirement.details.ext = sample();
        requirement.details.body = requirement_body.map(str::to_owned);
        codec::write(&document)
    };
    let base = with(None, "Keys address nodes.");
    let (path, _) = feature_file(&base);
    let ours = with(Some("Ours."), "Keys address nodes.");
    let theirs = with(None, "Keys address every node.");
    let merged = merge_file(&path, &base[&path], &ours[&path], &theirs[&path]).unwrap();
    assert_eq!(merged.conflicts, 0);
    let both = with(Some("Ours."), "Keys address every node.");
    assert_eq!(merged.bytes, both[&path]);
}

#[test]
fn the_extensions_root_is_the_digest_of_each_extended_node_key_and_ext_sorted_by_key() {
    let mut document = full_nested_document();
    let mut walked = 0;
    every_node(&mut document, &mut |ext| {
        walked += 1;
        ext.insert(
            Namespace::parse("tool").unwrap(),
            ExtValue::Map(BTreeMap::from([
                ("zeta".to_owned(), ExtValue::Integer(walked)),
                ("alpha".to_owned(), ExtValue::Boolean(walked % 2 == 0)),
            ])),
        );
    });
    let mut view = Vec::new();
    document.walk(&mut |node| {
        if let Some(id) = node.id() {
            view.push(json!([id.key().to_string(), ext::to_json(node.ext())]));
        }
    });
    let walk_order = view.clone();
    view.sort_by(|a, b| a[0].as_str().cmp(&b[0].as_str()));
    assert_ne!(
        walk_order, view,
        "the document's walk order is its key order"
    );
    let index = document.index().unwrap();
    let (hashes, views) = hash_indexed_with_views(&index);
    let extensions = hashes.roots.extensions.unwrap();
    assert_eq!(extensions, digest(Domain::Extensions, &view));
    let (_, _, state) = views
        .roots
        .iter()
        .find(|(name, ..)| *name == "state")
        .unwrap();
    let state = state.as_array().unwrap();
    assert_eq!(state.len(), 7);
    assert_eq!(state[6], json!(extensions.to_string()));
}

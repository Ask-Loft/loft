//! Own record views (Own record views): one for each keyed node, decision, registry entry and
//! binding, each the record as the writer writes it in canonical layout, without its key and
//! without the records it holds.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::codec::{self, RecordKind};
use loftspec::document::Document;
use loftspec::ext::ExtValue;
use loftspec::identity::Key;
use loftspec::ids::RequirementId;
use loftspec::names::{Namespace, TagName};
use loftspec::statement::Statement;
use serde_json::{Value, json};

use crate::common::full::{full_document, full_nested_document};
use crate::common::own::{
    COMPONENT, CONSTRAINT, CRITERION, FEATURE, GROUP, PRODUCT, QUALITY, REQUIREMENT, own, view,
};
use crate::common::text::{SHALL_CONFLICT, on_requirement};
use crate::common::{key, label, small_document, term};

fn text(n: u32) -> Value {
    json!(key(n).to_string())
}

/// The full nested document with a term list, `ext` on its Feature and a repeated tag on its
/// Product: every field family the writer writes.
fn every_field() -> Document {
    let mut document = full_nested_document();
    document.terms = vec![term(70, "record", &["row", "entry"])];
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.details.ext =
        BTreeMap::from([(Namespace::parse("tool").unwrap(), ExtValue::Integer(3))]);
    let tags = &mut document.products[0].details.tags;
    tags.extend([TagName::parse("portable").unwrap(), tags[0].clone()]);
    document
}

/// The keys of the own records of the kinds `wanted` accepts.
fn keys_of(document: &Document, wanted: fn(RecordKind) -> bool) -> BTreeSet<(RecordKind, Key)> {
    own(document)
        .into_keys()
        .filter(|(kind, _)| wanted(*kind))
        .collect()
}

/// The keys of the own records of one kind.
fn keys_of_kind(document: &Document, kind: RecordKind) -> BTreeSet<Key> {
    own(document)
        .into_keys()
        .filter_map(|(each, key)| (each == kind).then_some(key))
        .collect()
}

#[test]
fn every_keyed_node_and_every_decision_has_one_own_record_view() {
    let document = every_field();
    let mut expected = BTreeSet::new();
    document.walk(&mut |node| {
        if let Some(id) = node.id() {
            expected.insert((RecordKind::Node(id.kind()), id.key()));
        }
    });
    for decision in &document.decisions {
        expected.insert((RecordKind::Decision, decision.id.key()));
    }
    let wanted = |kind| matches!(kind, RecordKind::Node(_) | RecordKind::Decision);
    assert_eq!(keys_of(&document, wanted), expected);
}

#[test]
fn every_registry_entry_has_one_own_record_view() {
    let document = every_field();
    let tags: BTreeSet<Key> = document.tags.iter().map(|tag| tag.id.key()).collect();
    let categories = document.categories.iter().map(|category| category.id.key());
    assert_eq!(keys_of_kind(&document, RecordKind::Tag), tags);
    assert_eq!(
        keys_of_kind(&document, RecordKind::Category),
        categories.collect()
    );
    assert_eq!(
        keys_of_kind(&document, RecordKind::Term),
        BTreeSet::from([key(70)])
    );
}

#[test]
fn every_binding_departed_or_not_has_one_own_record_view_beside_its_elements() {
    let document = full_document();
    let bindings = keys_of_kind(&document, RecordKind::Binding);
    let expected: BTreeSet<Key> = document
        .bindings
        .iter()
        .map(|binding| binding.element.key())
        .collect();
    assert_eq!(bindings, expected);
    assert!(bindings.contains(&key(90)));
    let records = own(&document);
    assert!(records.contains_key(&(REQUIREMENT, key(4))));
    assert!(records.contains_key(&(RecordKind::Binding, key(4))));
}

#[test]
fn a_node_without_a_key_has_no_own_record_view() {
    let mut document = small_document();
    let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
    requirement.criteria[0].id = None;
    document.edges.retain(|edge| edge.source().key() != key(5));
    let records = own(&document);
    assert!(!records.contains_key(&(CRITERION, key(5))));
    let criteria = records
        .keys()
        .filter(|(kind, _)| *kind == CRITERION)
        .count();
    assert_eq!(criteria, 0);
    assert_eq!(records.len(), own(&small_document()).len() - 1);
}

#[test]
fn a_decisions_drivers_options_and_consequences_have_no_own_record_view() {
    let records = own(&small_document());
    for n in 21..=26 {
        assert!(records.keys().all(|(_, each)| *each != key(n)), "{n}");
    }
}

#[test]
fn a_document_written_as_files_and_read_back_has_the_own_record_views_it_had() {
    let document = every_field();
    let read = codec::read(&codec::write(&document)).unwrap();
    assert_eq!(own(&read), own(&document));
}

#[test]
fn the_own_record_view_is_an_object_of_the_fields_the_writer_writes_by_name() {
    let expected = json!({
        "name": "Every node record carries a `key` field.",
        "mentions": [text(4)],
    });
    assert_eq!(view(&small_document(), CRITERION, 5), expected);
}

#[test]
fn an_empty_list_the_writer_leaves_out_is_left_out_of_the_own_record_view() {
    let document = small_document();
    assert!(view(&document, GROUP, 2).get("tags").is_none());
    assert!(view(&document, FEATURE, 3).get("mentions").is_none());
    assert_eq!(view(&document, PRODUCT, 1)["tags"], json!(["portable"]));
}

#[test]
fn no_own_record_view_holds_its_records_key() {
    for record in own(&every_field()).values() {
        assert!(record.view.get("key").is_none(), "{:?}", record.view);
    }
}

#[test]
fn the_own_record_view_leaves_out_the_records_of_the_nodes_the_record_holds() {
    let document = every_field();
    let left_out = [
        (
            PRODUCT,
            1,
            &["featureGroups", "constraints", "sections"][..],
        ),
        (GROUP, 2, &["constraints", "qualityAttributes", "sections"]),
        (
            FEATURE,
            3,
            &["requirements", "qualityAttributes", "sections"],
        ),
        (REQUIREMENT, 4, &["criteria", "sections"]),
        (
            COMPONENT,
            6,
            &["components", "constraints", "qualityAttributes"],
        ),
        (
            COMPONENT,
            61,
            &["components", "constraints", "qualityAttributes"],
        ),
    ];
    for (kind, n, fields) in left_out {
        let view = view(&document, kind, n);
        for field in fields {
            assert!(view.get(field).is_none(), "{field} of {n}: {view}");
        }
    }
}

#[test]
fn a_feature_groups_own_record_view_leaves_out_its_features_keys() {
    let view = view(&small_document(), GROUP, 2);
    assert_eq!(view, json!({"slug": "identity", "name": "Identity"}));
}

#[test]
fn the_own_record_view_leaves_out_the_conflicts_its_record_carries() {
    let conflicted = codec::read(&on_requirement(SHALL_CONFLICT)).unwrap();
    assert_ne!(
        conflicted.conflicts,
        [] as [loftspec::conflict::Conflict; 0]
    );
    let clean = small_document();
    assert_eq!(
        view(&conflicted, REQUIREMENT, 4),
        view(&clean, REQUIREMENT, 4)
    );
}

#[test]
fn the_own_record_view_holds_the_slug_of_each_record_that_has_one() {
    let document = every_field();
    let slugs = [
        (PRODUCT, 1, "example-product"),
        (GROUP, 2, "identity"),
        (FEATURE, 3, "keys"),
        (COMPONENT, 6, "document-model"),
        (RecordKind::Decision, 20, "keys-are-uuids"),
    ];
    for (kind, n, slug) in slugs {
        assert_eq!(view(&document, kind, n)["slug"], json!(slug), "{n}");
    }
    assert!(view(&document, COMPONENT, 61).get("slug").is_none());
}

#[test]
fn the_own_record_view_holds_a_nodes_name_and_body() {
    let view = view(&small_document(), FEATURE, 3);
    assert_eq!(view["name"], json!("Keys"));
    assert_eq!(view["body"], json!("Keys address nodes."));
}

#[test]
fn the_own_record_view_holds_a_requirements_statement_as_the_writer_writes_it() {
    let mut document = small_document();
    let form = json!({"form": "ubiquitous", "system": "The writer", "shall": "write a key on every node."});
    assert_eq!(view(&document, REQUIREMENT, 4)["statement"], form);
    let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
    requirement.statement = Statement::Text("Every node carries a key.".to_owned());
    let text = json!({"text": "Every node carries a key."});
    assert_eq!(view(&document, REQUIREMENT, 4)["statement"], text);
}

#[test]
fn the_own_record_view_holds_tags_in_the_authored_order_with_duplicates() {
    let view = view(&every_field(), PRODUCT, 1);
    assert_eq!(view["tags"], json!(["identity", "portable", "identity"]));
}

#[test]
fn the_own_record_view_holds_the_edges_its_record_writes() {
    let document = full_nested_document();
    assert_eq!(view(&document, FEATURE, 3)["realizes"], json!([text(6)]));
    assert_eq!(view(&document, COMPONENT, 61)["scopedTo"], json!([text(1)]));
    assert_eq!(view(&document, CRITERION, 5)["mentions"], json!([text(4)]));
    let subjects = json!([
        {"role": "affected", "node": text(6)},
        {"role": "outcome", "node": text(4)},
    ]);
    let decision = view(&document, RecordKind::Decision, 20);
    assert_eq!(decision["subjects"], subjects);
    assert_eq!(decision["mentions"], json!([text(4)]));
}

#[test]
fn the_own_record_view_holds_a_nodes_ext() {
    assert_eq!(view(&every_field(), FEATURE, 3)["ext"], json!({"tool": 3}));
}

#[test]
fn the_own_record_view_holds_the_typed_attributes_the_writer_writes() {
    let document = every_field();
    assert_eq!(view(&document, COMPONENT, 6)["boundary"], json!("private"));
    let constraint = view(&document, CONSTRAINT, 7);
    let binding = json!({"category": "runtime.language", "requiredValue": "rust"});
    assert_eq!(constraint["constraintBinding"], binding);
    assert_eq!(constraint["verificationFate"], json!("proved"));
    assert_eq!(
        view(&document, CONSTRAINT, 7)["conservationKind"],
        json!("cap")
    );
    let quality = view(&document, QUALITY, 8);
    assert_eq!(quality["qaKind"], json!("latency"));
    assert_eq!(quality["qualityTarget"]["metric"], json!("fmt.wall-time"));
    assert_eq!(quality["qualityTarget"]["band"]["upper"], json!("2"));
    assert_eq!(quality["verificationFate"], json!("sampled"));
}

#[test]
fn the_own_record_view_holds_a_decisions_title_context_and_state() {
    let view = view(&small_document(), RecordKind::Decision, 20);
    assert_eq!(view["title"], json!("Keys are UUIDs"));
    assert_eq!(
        view["context"],
        json!("Every node carries its key in the file.")
    );
    let state = json!({"kind": "accepted", "selectedOption": text(23), "outcome": "New keys are UUID version 7."});
    assert_eq!(view["state"], state);
}

#[test]
fn the_own_record_view_holds_a_decisions_parts_with_their_keys() {
    let view = view(&small_document(), RecordKind::Decision, 20);
    let keys = |field: &str| -> Vec<Value> {
        let parts = view[field].as_array().unwrap();
        parts.iter().map(|part| part["key"].clone()).collect()
    };
    assert_eq!(keys("drivers"), [text(21), text(22)]);
    assert_eq!(keys("options"), [text(23), text(24)]);
    assert_eq!(keys("consequences"), [text(25), text(26)]);
    assert_eq!(view["assessments"].as_array().unwrap().len(), 4);
    assert_eq!(view["drivers"][1]["subject"], text(4));
    assert_eq!(view["options"][1]["title"], json!("ULID"));
}

#[test]
fn the_own_record_view_holds_a_registry_entrys_name_definition_and_sorted_aliases() {
    let document = every_field();
    let tag = json!({"name": "identity", "definition": "Keys, labels, aliases and the bindings between them."});
    assert_eq!(view(&document, RecordKind::Tag, 30), tag);
    let category = view(&document, RecordKind::Category, 32);
    assert_eq!(category["name"], json!("runtime.language"));
    let term =
        json!({"name": "record", "definition": "What record means.", "aliases": ["entry", "row"]});
    assert_eq!(view(&document, RecordKind::Term, 70), term);
}

#[test]
fn the_own_record_view_holds_a_bindings_fields_as_the_writer_writes_them() {
    let document = full_document();
    let departed = json!({
        "label": "ADR-5",
        "departed": true,
        "aliases": ["ADR-4"],
        "withdrawn": ["ADR-3"],
        "publishedState": "accepted",
        "publishedHistory": "cd".repeat(32),
    });
    assert_eq!(view(&document, RecordKind::Binding, 90), departed);
    let requirement = json!({"label": "RQ-2", "aliases": ["RQ-1"], "withdrawn": ["RQ-7"]});
    assert_eq!(view(&document, RecordKind::Binding, 4), requirement);
    let mut relabelled = document;
    let id = RequirementId::new(key(4));
    relabelled.bindings.relabel(id, label("RQ-3")).unwrap();
    assert_eq!(
        view(&relabelled, RecordKind::Binding, 4)["label"],
        json!("RQ-3")
    );
}

/// Whether every scalar outside `ext` is a string or `true`.
fn strings_or_true(value: &Value) -> bool {
    match value {
        Value::String(_) | Value::Bool(true) => true,
        Value::Array(items) => items.iter().all(strings_or_true),
        Value::Object(fields) => fields
            .iter()
            .all(|(name, field)| name == "ext" || strings_or_true(field)),
        Value::Null | Value::Bool(false) | Value::Number(_) => false,
    }
}

#[test]
fn outside_ext_every_scalar_of_an_own_record_view_is_a_string_or_true() {
    let records = own(&every_field());
    for record in records.values() {
        assert!(strings_or_true(&record.view), "{}", record.view);
    }
    let departed = &records[&(RecordKind::Binding, key(90))].view;
    assert_eq!(departed["departed"], json!(true));
}

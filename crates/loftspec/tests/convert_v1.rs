//! Converting a Loft v1 export into a document (D-11 F-13, F-14): node kinds, keys and labels,
//! parentage and order, Facet Tags, the registries, statements and the label counters.

mod common;

use common::v1::{export, node, uuid};
use loftspec::convert::v1::export::Export;
use loftspec::convert::v1::{Conversion, ConvertError, Finding, convert};
use loftspec::hash::hash_document;
use loftspec::identity::{Key, Label, LabelPrefix};
use loftspec::ids::{
    CategoryId, ComponentId, ConstraintId, DocumentId, FeatureGroupId, FeatureId, ProductId,
    QualityAttributeId, RequirementId, TagId,
};
use loftspec::names::TagName;
use loftspec::statement::Statement;
use serde_json::{Value, json};

fn key(n: u32) -> Key {
    Key::parse(&uuid(n)).unwrap()
}

fn named(mut node: Value, name: &str, ordinal: i64) -> Value {
    node["name"] = json!(name);
    node["ordinal"] = json!(ordinal);
    node
}

fn tier(name: &str) -> Value {
    json!(name)
}

fn constraint(n: u32, label: &str) -> Value {
    let mut constraint = node(n, "Constraint", &Value::Null, label, &[]);
    constraint["verificationFate"] = json!("monitored");
    constraint["fateConfirmed"] = json!(true);
    constraint["technicalBinding"] =
        json!({"category": "runtime.language", "requiredValue": "rust"});
    named(constraint, &format!("Constraint {n} shall hold."), 0)
}

fn quality(n: u32, label: &str) -> Value {
    let mut quality = node(n, "QualityAttribute", &Value::Null, label, &[]);
    quality["qaKind"] = json!("latency");
    quality["verificationFate"] = json!("sampled");
    quality["qualityTarget"] = json!({
        "metric": "check.wall-time", "band": {"kind": "at_most", "upper": "2"}, "unit": "s",
        "aggregation": "p95", "scenario": "a check", "environment": "a machine",
    });
    named(quality, &format!("Quality {n} is quick."), 1)
}

/// P-1 (tag `portable`) > FG-1 > F-1 > RQ-1 (slots) and RQ-2 (text), listed out of ordinal
/// order, with AC-1 under RQ-1 and QA-2 under F-1; CON-2 under FG-1; root CMP-1 (tag `identity`)
/// with CON-1, QA-1 and CMP-2.
fn tree() -> Vec<Value> {
    vec![
        named(
            node(1, "TierSection", &tier("Product"), "P-1", &[2, 9]),
            "Example",
            0,
        ),
        named(
            node(2, "TierSection", &tier("FeatureGroup"), "FG-1", &[3, 11]),
            "Identity",
            0,
        ),
        named(
            node(3, "TierSection", &tier("Feature"), "F-1", &[13, 4, 12]),
            "Keys",
            0,
        ),
        named(
            node(4, "TierSection", &tier("Requirement"), "RQ-1", &[5]),
            "The writer shall write a key on every node.",
            0,
        ),
        named(
            node(5, "AcceptanceCriterion", &Value::Null, "AC-1", &[]),
            "Every node record carries a key.",
            0,
        ),
        named(
            node(6, "Component", &Value::Null, "CMP-1", &[7, 8, 10, 14]),
            "Document model",
            0,
        ),
        constraint(7, "CON-1"),
        quality(8, "QA-1"),
        named(
            node(9, "FacetTag", &Value::Null, "FT-1", &[]),
            "portable",
            0,
        ),
        named(node(10, "Component", &Value::Null, "CMP-2", &[]), "Keys", 2),
        constraint(11, "CON-2"),
        quality(12, "QA-2"),
        named(
            node(13, "TierSection", &tier("Requirement"), "RQ-2", &[]),
            "Keys are UUIDs",
            1,
        ),
        named(
            node(14, "FacetTag", &Value::Null, "FT-2", &[]),
            "identity",
            3,
        ),
    ]
}

fn with_identity_entry(mut value: Value) -> Value {
    value["vocabularyEntries"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "entryHash": "ab".repeat(32), "entryId": uuid(72), "documentId": uuid(0),
            "name": "identity", "definition": "Keys and labels.",
        }));
    value
}

fn read(value: &Value) -> Export {
    Export::from_json(&value.to_string()).unwrap()
}

fn converted() -> Conversion {
    convert(&read(&with_identity_entry(export(&tree(), &[])))).unwrap()
}

#[test]
fn ac_162_to_ac_164_and_ac_171_each_node_maps_to_its_kind_under_its_parent() {
    let document = converted().document;
    let [product] = document.products.as_slice() else {
        panic!("one Product");
    };
    assert_eq!(product.id, Some(ProductId::new(key(1))));
    let group = &product.feature_groups[0];
    assert_eq!(group.id, Some(FeatureGroupId::new(key(2))));
    let feature = &group.features[0];
    assert_eq!(feature.id, Some(FeatureId::new(key(3))));
    assert_eq!(feature.requirements[0].criteria.len(), 1);
    let [component] = document.components.as_slice() else {
        panic!("one root Component");
    };
    assert_eq!(component.id, Some(ComponentId::new(key(6))));
    assert_eq!(component.components[0].id, Some(ComponentId::new(key(10))));
    assert_eq!(document.id, DocumentId::new(key(0)));
}

#[test]
fn ac_165_and_ac_166_constraints_and_quality_attributes_keep_their_fields() {
    let document = converted().document;
    let component = &document.components[0];
    let constraint = &component.constraints[0];
    assert_eq!(constraint.id, Some(ConstraintId::new(key(7))));
    assert_eq!(constraint.binding.as_ref().unwrap().required_value, "rust");
    assert!(constraint.verification_fate.is_some());
    let quality = &component.quality_attributes[0];
    assert_eq!(quality.id, Some(QualityAttributeId::new(key(8))));
    assert_eq!(quality.target.metric, "check.wall-time");
}

#[test]
fn constraints_and_quality_attributes_keep_their_parent_kind() {
    let document = converted().document;
    let group = &document.products[0].feature_groups[0];
    assert_eq!(group.constraints[0].id, Some(ConstraintId::new(key(11))));
    assert_eq!(
        group.features[0].quality_attributes[0].id,
        Some(QualityAttributeId::new(key(12)))
    );
}

#[test]
fn ac_167_the_node_count_is_the_v1_count_less_the_facet_tags() {
    let document = converted().document;
    let hashes = hash_document(&document).unwrap();
    assert_eq!(hashes.nodes.len(), tree().len() - 2);
}

#[test]
fn ac_168_to_ac_170_every_node_keeps_its_uuid_and_its_external_id_without_aliases() {
    let document = converted().document;
    for v1 in tree() {
        if v1["type"] == "FacetTag" {
            continue;
        }
        let key = Key::parse(v1["nodeId"].as_str().unwrap()).unwrap();
        let binding = document
            .bindings
            .iter()
            .find(|binding| binding.element.key() == key)
            .unwrap_or_else(|| panic!("{key} is bound"));
        assert_eq!(
            binding.label,
            Label::parse(v1["externalId"].as_str().unwrap()).unwrap()
        );
        assert!(binding.aliases.is_empty());
    }
    assert_eq!(document.bindings.len(), tree().len() - 2);
}

#[test]
fn ac_173_and_ac_174_a_facet_tag_becomes_a_tag_on_its_parent() {
    let document = converted().document;
    assert_eq!(
        document.products[0].details.tags,
        [TagName::parse("portable").unwrap()]
    );
    assert_eq!(
        document.components[0].details.tags,
        [TagName::parse("identity").unwrap()]
    );
    let hashes = hash_document(&document).unwrap();
    assert!(hashes.nodes.keys().all(|id| id.key() != key(9)));
}

#[test]
fn ac_175_ac_176_and_ac_226_to_ac_228_the_registries_keep_keys_names_and_definitions() {
    let document = converted().document;
    let tags: Vec<(TagId, &str, &str)> = document
        .tags
        .iter()
        .map(|entry| (entry.id, entry.name.as_str(), entry.definition.as_str()))
        .collect();
    assert_eq!(
        tags,
        [
            (
                TagId::new(key(71)),
                "portable",
                "Behaviour every implementation reproduces."
            ),
            (TagId::new(key(72)), "identity", "Keys and labels."),
        ]
    );
    assert_eq!(document.categories.len(), 1);
    assert_eq!(document.categories[0].id, CategoryId::new(key(70)));
    assert_eq!(document.categories[0].name.as_str(), "runtime.language");
}

#[test]
fn ac_236_siblings_are_ordered_by_ascending_ordinal() {
    let document = converted().document;
    let ids: Vec<Option<RequirementId>> = document.products[0].feature_groups[0].features[0]
        .requirements
        .iter()
        .map(|requirement| requirement.id)
        .collect();
    assert_eq!(
        ids,
        [
            Some(RequirementId::new(key(4))),
            Some(RequirementId::new(key(13)))
        ]
    );
}

#[test]
fn ac_185_to_ac_187_a_statement_is_slots_when_it_renders_back_and_text_with_a_finding_otherwise() {
    let conversion = converted();
    let requirements = &conversion.document.products[0].feature_groups[0].features[0].requirements;
    assert!(matches!(requirements[0].statement, Statement::Form(_)));
    assert_eq!(
        requirements[1].statement,
        Statement::Text("Keys are UUIDs".to_owned())
    );
    assert!(conversion.findings.contains(&Finding::StatementAsText {
        requirement: RequirementId::new(key(13))
    }));
}

#[test]
fn ac_702_each_counter_is_one_above_the_highest_label_and_an_unused_prefix_starts_at_1() {
    let counters = converted().document.counters;
    assert_eq!(counters.next(LabelPrefix::Requirement).get(), 3);
    assert_eq!(counters.next(LabelPrefix::Component).get(), 3);
    assert_eq!(counters.next(LabelPrefix::Product).get(), 2);
    assert_eq!(counters.next(LabelPrefix::Decision).get(), 1);
}

#[test]
fn an_export_naming_no_document_is_refused() {
    let mut value = export(&[], &[]);
    value["vocabularyEntries"] = json!([]);
    value["technicalConstraintCategories"] = json!([]);
    value["realizationLinks"] = json!([]);
    assert!(matches!(
        convert(&read(&value)),
        Err(ConvertError::NoDocument)
    ));
}

#[test]
fn an_export_naming_two_documents_is_refused() {
    let mut nodes = tree();
    nodes[4]["documentId"] = json!(uuid(98));
    assert!(matches!(
        convert(&read(&with_identity_entry(export(&nodes, &[])))),
        Err(ConvertError::TwoDocuments { .. })
    ));
}

#[test]
fn a_child_outside_the_containment_profile_is_refused_naming_both() {
    let mut nodes = tree();
    // AC-1 listed directly under F-1.
    nodes[2] = named(
        node(3, "TierSection", &tier("Feature"), "F-1", &[13, 4, 12, 5]),
        "Keys",
        0,
    );
    nodes[3] = named(
        node(4, "TierSection", &tier("Requirement"), "RQ-1", &[]),
        "The writer shall write a key on every node.",
        0,
    );
    let error = convert(&read(&with_identity_entry(export(&nodes, &[])))).unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("F-1") && message.contains("AC-1"),
        "{message}"
    );
}

#[test]
fn a_node_no_parent_lists_is_refused_unless_it_is_a_root() {
    let mut nodes = tree();
    // RQ-2 is no longer listed by F-1.
    nodes[2] = named(
        node(3, "TierSection", &tier("Feature"), "F-1", &[4, 12]),
        "Keys",
        0,
    );
    let error = convert(&read(&with_identity_entry(export(&nodes, &[])))).unwrap_err();
    assert!(error.to_string().contains("RQ-2"), "{error}");
}

#[test]
fn an_annotation_is_refused() {
    let mut value = with_identity_entry(export(&tree(), &[]));
    value["annotations"] = json!([{"note": "x"}]);
    assert!(matches!(
        convert(&read(&value)),
        Err(ConvertError::Annotations)
    ));
}

#[test]
#[ignore = "reads a local v1 export named by LOFTSPEC_V1_EXPORT; no export is committed here"]
fn a_live_v1_export_converts() {
    let path = std::env::var("LOFTSPEC_V1_EXPORT").expect("LOFTSPEC_V1_EXPORT names an export");
    let export = Export::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    let conversion = convert(&export).unwrap();
    let facet_tags = export
        .nodes
        .iter()
        .filter(|node| node.node_type == loftspec::convert::v1::export::NodeType::FacetTag)
        .count();
    let hashes = hash_document(&conversion.document).unwrap();
    assert_eq!(hashes.nodes.len(), export.nodes.len() - facet_tags);
    println!(
        "{} nodes, {} findings",
        hashes.nodes.len(),
        conversion.findings.len()
    );
}

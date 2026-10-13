//! Converting a Loft v1 export into a document (D-11 F-13, F-14): node kinds, keys and labels,
//! parentage and order, Facet Tags, the registries, statements and the label counters.

mod common;

use common::v1::{
    export, key, live_export, named, node, read, tier, tree, uuid, with_identity_entry,
};
use loftspec::convert::v1::{Conversion, ConvertError, Finding, convert};
use loftspec::hash::hash_document;
use loftspec::identity::{Key, Label, LabelPrefix};
use loftspec::ids::{
    CategoryId, ComponentId, ConstraintId, DocumentId, FeatureGroupId, FeatureId, ProductId,
    QualityAttributeId, RequirementId, TagId,
};
use loftspec::names::TagName;
use loftspec::statement::Statement;
use serde_json::json;

fn converted() -> Conversion {
    convert(&read(&with_identity_entry(export(&tree(), &[])))).unwrap()
}

#[test]
fn each_node_maps_to_its_kind_under_its_parent() {
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
fn constraints_and_quality_attributes_keep_their_fields() {
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
fn the_node_count_is_the_v1_count_less_the_facet_tags() {
    let document = converted().document;
    let hashes = hash_document(&document).unwrap();
    assert_eq!(hashes.nodes.len(), tree().len() - 2);
}

#[test]
fn every_node_keeps_its_uuid_and_its_external_id_without_aliases() {
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
        assert_eq!(binding.aliases, Vec::new());
    }
    assert_eq!(document.bindings.len(), tree().len() - 2);
}

#[test]
fn a_facet_tag_becomes_a_tag_on_its_parent() {
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
fn the_registries_keep_keys_names_and_definitions() {
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
fn siblings_are_ordered_by_ascending_ordinal() {
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
fn a_statement_is_slots_when_it_renders_back_and_text_with_a_finding_otherwise() {
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
fn each_counter_is_one_above_the_highest_label_and_an_unused_prefix_starts_at_1() {
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
    let export = live_export();
    let conversion = convert(&export).unwrap();
    let facet_tags = export
        .nodes
        .iter()
        .filter(|node| node.node_type == loftspec::convert::v1::export::NodeType::FacetTag)
        .count();
    let hashes = hash_document(&conversion.document).expect("a converted export hashes");
    println!("state root {}", hashes.roots.state);
    let sidecar = conversion.sidecar.to_json();
    assert_eq!(sidecar, convert(&export).unwrap().sidecar.to_json());
    assert_eq!(
        conversion.sidecar.nodes.len(),
        export.nodes.len() - facet_tags
    );
    let nodes = conversion.document.bindings.len() - conversion.document.decisions.len();
    assert_eq!(nodes, export.nodes.len() - facet_tags);
    assert_eq!(
        conversion.document.decisions.len(),
        export.architecture_decisions.len()
    );
    println!(
        "{nodes} nodes, {} edges, {} decisions, {} former labels, sidecar {} bytes, findings: {:?}",
        conversion.document.edges.len(),
        conversion.document.decisions.len(),
        conversion.sidecar.former_labels.len(),
        sidecar.len(),
        conversion.findings.iter().fold(
            std::collections::BTreeMap::new(),
            |mut counts, finding| {
                *counts
                    .entry(
                        format!("{finding:?}")
                            .split([' ', '{'])
                            .next()
                            .unwrap()
                            .to_owned(),
                    )
                    .or_insert(0) += 1;
                counts
            }
        )
    );
}

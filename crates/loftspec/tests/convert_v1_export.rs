//! Reading a Loft v1 Authored State export (D-11 F-13), the converter's input. The reader is
//! strict: a field the v1 export does not define is an error, so a drift in the v1 format
//! surfaces here and not as a wrong conversion.

use loftspec::attributes::{QaKind, QualityAggregation, QualityBand, VerificationFate};
use loftspec::convert::v1::export::{
    ComponentScope, ConsequenceKind, DecisionState, DriverPayload, ElementPayload, Export,
    NodeType, Rating, SubjectRole, Tier,
};
use serde_json::{Value, json};

mod common;

use common::v1::{export, hash, node, uuid};

const D11_SEQUENCE_1: &str =
    include_str!("../../../testdata/v1/d11-sequence-1/loft-authored-state.json");

fn read(value: &Value) -> Export {
    Export::from_json(&value.to_string()).unwrap()
}

#[test]
fn the_formats_own_first_export_reads() {
    let export = Export::from_json(D11_SEQUENCE_1).unwrap();
    assert_eq!(export.publish_sequence, 1);
    assert!(export.nodes.is_empty());
    assert_eq!(
        export.content_address.to_string(),
        "77b926758c7b38f0be81c137acb8bf4ca37500754f46ee55666a0a85c4661204"
    );
    assert_eq!(export.architecture_decision_address, None);
    assert_eq!(export.technical_constraint_category_address, None);
}

fn typed_nodes() -> Vec<Value> {
    let mut product = node(1, "TierSection", &json!("Product"), "P-1", &[2, 9]);
    product["body"] = json!("The product.");
    let mut component = node(6, "Component", &Value::Null, "CMP-1", &[7, 8]);
    component["componentScope"] = json!({"kind": "product", "productId": uuid(1)});
    let mut constraint = node(7, "Constraint", &Value::Null, "CON-1", &[]);
    constraint["verificationFate"] = json!("monitored");
    constraint["fateConfirmed"] = json!(true);
    constraint["technicalBinding"] =
        json!({"category": "runtime.language", "requiredValue": "rust"});
    let mut quality = node(8, "QualityAttribute", &Value::Null, "QA-1", &[]);
    quality["qaKind"] = json!("latency");
    quality["verificationFate"] = json!("sampled");
    quality["qualityTarget"] = json!({
        "metric": "check.wall-time", "band": {"kind": "at_most", "upper": "2"}, "unit": "s",
        "aggregation": "each", "scenario": "a check", "environment": "a machine",
    });
    vec![
        product,
        node(2, "TierSection", &json!("FeatureGroup"), "FG-1", &[3]),
        node(3, "TierSection", &json!("Feature"), "F-1", &[4]),
        node(4, "TierSection", &json!("Requirement"), "RQ-1", &[5]),
        node(5, "AcceptanceCriterion", &Value::Null, "AC-1", &[]),
        component,
        constraint,
        quality,
        node(9, "FacetTag", &Value::Null, "FT-1", &[]),
    ]
}

#[test]
fn every_node_type_and_tier_reads() {
    let export = read(&export(&typed_nodes(), &[]));
    let types: Vec<(NodeType, Option<Tier>)> = export
        .nodes
        .iter()
        .map(|node| (node.node_type, node.tier))
        .collect();
    assert_eq!(
        types,
        [
            (NodeType::TierSection, Some(Tier::Product)),
            (NodeType::TierSection, Some(Tier::FeatureGroup)),
            (NodeType::TierSection, Some(Tier::Feature)),
            (NodeType::TierSection, Some(Tier::Requirement)),
            (NodeType::AcceptanceCriterion, None),
            (NodeType::Component, None),
            (NodeType::Constraint, None),
            (NodeType::QualityAttribute, None),
            (NodeType::FacetTag, None),
        ]
    );
    assert_eq!(export.nodes[0].external_id.to_string(), "P-1");
    assert_eq!(export.nodes[8].external_id.prefix(), "FT");
    let children: Vec<String> = export.nodes[0]
        .children_hashes
        .iter()
        .map(|child| child.node_id.to_string())
        .collect();
    assert_eq!(children, [uuid(2), uuid(9)]);
}

#[test]
fn typed_attributes_read_field_for_field() {
    let export = read(&export(&typed_nodes(), &[]));
    assert!(matches!(
        export.nodes[5].component_scope,
        Some(ComponentScope::Product { .. })
    ));
    assert_eq!(export.nodes[6].fate_confirmed, Some(true));
    assert_eq!(
        export.nodes[6]
            .technical_binding
            .as_ref()
            .unwrap()
            .category
            .as_str(),
        "runtime.language"
    );
    let target = export.nodes[7].quality_target.as_ref().unwrap();
    assert_eq!(export.nodes[7].qa_kind, Some(QaKind::Latency));
    assert_eq!(target.aggregation, QualityAggregation::Each);
    assert_eq!(
        target.band,
        QualityBand::AtMost {
            upper: "2".to_owned()
        }
    );
    assert_eq!(
        export.nodes[7].verification_fate,
        Some(VerificationFate::Sampled)
    );
}

#[test]
fn links_and_registries_read() {
    let export = read(&export(&typed_nodes(), &[]));
    assert_eq!(export.realization_links.len(), 1);
    assert_eq!(export.vocabulary_entries[0].name.as_str(), "portable");
    assert_eq!(
        export.technical_constraint_categories[0].name.as_str(),
        "runtime.language"
    );
}

fn subject(n: u32, external_id: &str) -> Value {
    json!({"externalId": external_id, "nodeId": uuid(n), "tier": "Requirement", "type": "TierSection"})
}

fn element(n: u32, parent: Option<u32>, ordinal: i64, payload: &Value) -> Value {
    json!({
        "documentId": uuid(0),
        "elementHash": hash(u8::try_from(n).unwrap()),
        "id": uuid(n),
        "ordinal": ordinal,
        "parentId": parent.map(uuid),
        "payload": payload,
    })
}

#[test]
fn a_decision_aggregate_reads() {
    let decision = json!({
        "identity": {"decisionId": uuid(20), "documentId": uuid(0), "externalId": "ADR-1"},
        "decisionHash": hash(20),
        "elements": [
            element(20, None, 0, &json!({"kind": "architecture-decision", "value": {
                "title": "Keys are UUIDs", "context": "Every node carries its key.",
                "state": {"kind": "superseded", "selectedOptionId": uuid(23),
                          "outcome": "Version 7.", "successorDecisionId": uuid(30)},
            }})),
            element(23, Some(20), 0, &json!({"kind": "option", "value": {"title": "UUID", "description": "RFC 9562."}})),
            element(21, Some(20), 0, &json!({"kind": "driver", "value": {"kind": "local", "label": "D1", "statement": "One shape."}})),
            element(22, Some(20), 1, &json!({"kind": "driver", "value": {"kind": "specification-subject", "subject": subject(4, "RQ-1")}})),
            element(24, Some(20), 0, &json!({"kind": "consequence", "value": {"kind": "risk", "statement": "A key reveals time.", "subject": null}})),
            element(25, Some(20), 1, &json!({"kind": "consequence", "value": {"kind": "obligation", "statement": "Mint keys.", "subject": subject(4, "RQ-1")}})),
        ],
        "assessments": [{"decisionId": uuid(20), "optionId": uuid(23), "driverId": uuid(21), "rating": "unknown", "rationale": "Not known."}],
        "subjectLinks": [{"decisionId": uuid(20), "role": "outcome", "subject": subject(4, "RQ-1")}],
    });
    let export = read(&export(&[], &[decision]));
    let decision = &export.architecture_decisions[0];
    assert_eq!(decision.identity.external_id.to_string(), "ADR-1");
    let kinds: Vec<&str> = decision
        .elements
        .iter()
        .map(|element| match &element.payload {
            ElementPayload::ArchitectureDecision { .. } => "decision",
            ElementPayload::DecisionOption { .. } => "option",
            ElementPayload::Driver(DriverPayload::Local { .. }) => "local driver",
            ElementPayload::Driver(DriverPayload::SpecificationSubject { .. }) => "subject driver",
            ElementPayload::Consequence { .. } => "consequence",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "decision",
            "option",
            "local driver",
            "subject driver",
            "consequence",
            "consequence"
        ]
    );
    let ElementPayload::ArchitectureDecision { state, .. } = &decision.elements[0].payload else {
        panic!("the root element is the decision");
    };
    assert!(matches!(state, DecisionState::Superseded { .. }));
    let ElementPayload::Consequence { kind, subject, .. } = &decision.elements[4].payload else {
        panic!("a consequence");
    };
    assert_eq!((*kind, subject.is_none()), (ConsequenceKind::Risk, true));
    assert_eq!(decision.assessments[0].rating, Rating::Unknown);
    assert_eq!(decision.subject_links[0].role, SubjectRole::Outcome);
}

#[test]
fn a_field_the_v1_export_does_not_define_is_an_error() {
    let mut value = export(
        &[node(1, "TierSection", &json!("Product"), "P-1", &[])],
        &[],
    );
    value["nodes"][0]["colour"] = json!("blue");
    assert!(Export::from_json(&value.to_string()).is_err());
}

#[test]
fn an_unknown_node_type_is_an_error() {
    let value = export(&[node(1, "Widget", &Value::Null, "W-1", &[])], &[]);
    assert!(Export::from_json(&value.to_string()).is_err());
}

#[test]
#[ignore = "reads a local v1 export named by LOFTSPEC_V1_EXPORT; no export is committed here"]
fn a_live_v1_export_reads() {
    let path = std::env::var("LOFTSPEC_V1_EXPORT").expect("LOFTSPEC_V1_EXPORT names an export");
    let text = std::fs::read_to_string(path).unwrap();
    let export = Export::from_json(&text).unwrap();
    assert!(!export.nodes.is_empty());
}

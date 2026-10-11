//! Converting a v1 export's links and decisions (D-11 F-13): `realizes` and `scoped_to` edges,
//! Architecture Decisions with their elements and `subject` edges, and the derived-from record.

mod common;

use common::v1::{decisions, export, hash, tree, uuid, with_identity_entry};
use loftspec::canonical::Digest;
use loftspec::convert::v1::export::Export;
use loftspec::convert::v1::{Conversion, ConvertError, Finding, convert};
use loftspec::decision::{ConsequenceKind, DecisionState, DriverPayload, Rating};
use loftspec::document::{DerivedFrom, Edge, SubjectRole};
use loftspec::hash::hash_document;
use loftspec::identity::{Key, Label};
use loftspec::ids::{
    AuthoredStateId, ComponentId, DecisionId, FeatureId, NodeId, OptionId, ProductId, RequirementId,
};
use serde_json::{Value, json};

fn key(n: u32) -> Key {
    Key::parse(&uuid(n)).unwrap()
}

fn full_export() -> Value {
    let mut nodes = tree();
    nodes[5]["componentScope"] = json!({"kind": "product", "productId": uuid(1)});
    with_identity_entry(export(&nodes, &decisions()))
}

fn converted(value: &Value) -> Result<Conversion, ConvertError> {
    convert(&Export::from_json(&value.to_string()).unwrap())
}

fn edges(conversion: &Conversion) -> &[Edge] {
    &conversion.document.edges
}

#[test]
fn ac_177_and_ac_178_each_realization_link_is_a_realizes_edge() {
    let conversion = converted(&full_export()).unwrap();
    let realizes: Vec<&Edge> = edges(&conversion)
        .iter()
        .filter(|edge| matches!(edge, Edge::Realizes { .. }))
        .collect();
    assert_eq!(
        realizes,
        [&Edge::Realizes {
            feature: FeatureId::new(key(3)),
            component: ComponentId::new(key(6)),
        }]
    );
}

#[test]
fn ac_179_and_ac_180_only_a_component_scoped_to_a_product_has_a_scoped_to_edge() {
    let conversion = converted(&full_export()).unwrap();
    let scoped: Vec<&Edge> = edges(&conversion)
        .iter()
        .filter(|edge| matches!(edge, Edge::ScopedTo { .. }))
        .collect();
    assert_eq!(
        scoped,
        [&Edge::ScopedTo {
            component: ComponentId::new(key(6)),
            product: ProductId::new(key(1)),
        }]
    );
}

#[test]
fn ac_181_ac_182_ac_183_and_ac_256_a_decision_keeps_its_keys_state_option_and_outcome() {
    let conversion = converted(&full_export()).unwrap();
    let decision = &conversion.document.decisions[0];
    assert_eq!(decision.id, DecisionId::new(key(20)));
    assert_eq!(decision.title, "Keys are UUIDs");
    assert_eq!(
        decision.state,
        DecisionState::Accepted {
            selected_option: OptionId::new(key(23)),
            outcome: "Version 7.".to_owned(),
        }
    );
    let options: Vec<OptionId> = decision.options.iter().map(|option| option.id).collect();
    assert_eq!(
        options,
        [OptionId::new(key(23)), OptionId::new(key(24))],
        "options in ordinal order"
    );
    let superseded = &conversion.document.decisions[1];
    assert!(matches!(
        superseded.state,
        DecisionState::Superseded { successor, .. } if successor == DecisionId::new(key(20))
    ));
}

#[test]
fn ac_257_drivers_consequences_and_assessments_keep_their_fields() {
    let conversion = converted(&full_export()).unwrap();
    let decision = &conversion.document.decisions[0];
    assert!(matches!(
        &decision.drivers[0].payload,
        DriverPayload::Local { label, .. } if label == "D1"
    ));
    assert_eq!(
        decision.drivers[1].payload,
        DriverPayload::Subject {
            subject: NodeId::Requirement(RequirementId::new(key(4)))
        }
    );
    assert_eq!(decision.consequences[0].kind, ConsequenceKind::Obligation);
    assert_eq!(
        decision.consequences[0].subject,
        Some(NodeId::Requirement(RequirementId::new(key(4))))
    );
    assert_eq!(decision.consequences[1].subject, None);
    assert_eq!(decision.assessments.len(), 4);
    assert_eq!(decision.assessments[3].rating, Rating::Unknown);
}

#[test]
fn ac_184_each_subject_link_is_a_subject_edge_with_its_role() {
    let conversion = converted(&full_export()).unwrap();
    let subjects: Vec<&Edge> = edges(&conversion)
        .iter()
        .filter(|edge| matches!(edge, Edge::Subject { .. }))
        .collect();
    assert_eq!(
        subjects,
        [
            &Edge::Subject {
                decision: DecisionId::new(key(20)),
                subject: NodeId::Requirement(RequirementId::new(key(4))),
                role: SubjectRole::Outcome,
            },
            &Edge::Subject {
                decision: DecisionId::new(key(20)),
                subject: NodeId::Component(ComponentId::new(key(6))),
                role: SubjectRole::Affected,
            },
        ]
    );
}

#[test]
fn ac_244_every_decision_has_its_v1_external_id_as_label() {
    let conversion = converted(&full_export()).unwrap();
    let bindings = &conversion.document.bindings;
    assert_eq!(
        bindings.label_of(DecisionId::new(key(20))),
        Some(Label::parse("ADR-1").unwrap())
    );
    assert_eq!(
        bindings.label_of(DecisionId::new(key(30))),
        Some(Label::parse("ADR-2").unwrap())
    );
    assert_eq!(
        conversion
            .document
            .counters
            .next(loftspec::identity::LabelPrefix::Decision)
            .get(),
        3
    );
}

#[test]
fn the_derived_from_record_names_the_source_state() {
    let conversion = converted(&full_export()).unwrap();
    assert_eq!(
        conversion.document.derived_from,
        Some(DerivedFrom {
            authored_state_id: AuthoredStateId::new(key(99)),
            content_address: hash(91).parse::<Digest>().unwrap(),
            publish_sequence: 7,
        })
    );
}

#[test]
fn a_converted_document_hashes() {
    let conversion = converted(&full_export()).unwrap();
    let hashes = hash_document(&conversion.document).unwrap();
    assert_eq!(hashes.decisions.len(), 2);
}

#[test]
fn a_realization_link_whose_ends_are_not_a_feature_and_a_component_is_refused() {
    let mut value = full_export();
    value["realizationLinks"] = json!([{"featureId": uuid(4), "componentId": uuid(6)}]);
    let error = converted(&value).unwrap_err();
    assert!(matches!(error, ConvertError::Link { .. }), "{error}");
}

#[test]
fn a_subject_the_export_does_not_hold_is_kept_under_its_key_with_a_finding() {
    let mut value = full_export();
    value["architectureDecisions"][0]["subjectLinks"][0]["subject"]["nodeId"] = json!(uuid(77));
    let conversion = converted(&value).unwrap();
    let absent = NodeId::Requirement(RequirementId::new(key(77)));
    assert!(edges(&conversion).contains(&Edge::Subject {
        decision: DecisionId::new(key(20)),
        subject: absent,
        role: SubjectRole::Outcome,
    }));
    assert!(conversion.findings.contains(&Finding::AbsentSubject {
        decision: DecisionId::new(key(20)),
        subject: absent,
    }));
}

#[test]
fn a_subject_of_no_kind_the_format_holds_is_refused() {
    let mut value = full_export();
    value["architectureDecisions"][0]["subjectLinks"][0]["subject"]["type"] = json!("FacetTag");
    value["architectureDecisions"][0]["subjectLinks"][0]["subject"]["tier"] = json!(null);
    value["architectureDecisions"][0]["subjectLinks"][0]["subject"]["nodeId"] = json!(uuid(77));
    let error = converted(&value).unwrap_err();
    assert!(error.to_string().contains("ADR-1"), "{error}");
}

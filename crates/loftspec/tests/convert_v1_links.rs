//! Converting a v1 export's links and decisions (D-11 F-13): `realizes` and `scoped_to` edges,
//! Architecture Decisions with their elements and `subject` edges, and the derived-from record.

mod common;

use common::v1::{convert_value, full_export, hash, key, uuid};
use loftspec::attributes::Boundary;
use loftspec::canonical::Digest;
use loftspec::convert::v1::{Conversion, ConvertError, Finding};
use loftspec::decision::{ConsequenceKind, DecisionState, DriverPayload, NodeRef, Rating};
use loftspec::document::{DerivedFrom, Edge, SubjectRole};
use loftspec::hash::hash_document;
use loftspec::identity::Label;
use loftspec::ids::{
    AuthoredStateId, ComponentId, CriterionId, DecisionId, ElementId, FeatureId, NodeId, OptionId,
    ProductId, RequirementId,
};
use serde_json::json;

fn edges(conversion: &Conversion) -> &[Edge] {
    &conversion.document.edges
}

#[test]
fn ac_177_and_ac_178_each_realization_link_is_a_realizes_edge() {
    let conversion = convert_value(&full_export()).unwrap();
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
    let conversion = convert_value(&full_export()).unwrap();
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
    let conversion = convert_value(&full_export()).unwrap();
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
    let conversion = convert_value(&full_export()).unwrap();
    let decision = &conversion.document.decisions[0];
    assert!(matches!(
        &decision.drivers[0].payload,
        DriverPayload::Local { label, .. } if label == "D1"
    ));
    assert_eq!(
        decision.drivers[1].payload,
        DriverPayload::Subject {
            subject: RequirementId::new(key(4)).into()
        }
    );
    assert_eq!(decision.consequences[0].kind, ConsequenceKind::Obligation);
    assert_eq!(
        decision.consequences[0].subject,
        Some(RequirementId::new(key(4)).into())
    );
    assert_eq!(decision.consequences[1].subject, None);
    assert_eq!(decision.assessments.len(), 4);
    assert_eq!(decision.assessments[3].rating, Rating::Unknown);
}

#[test]
fn ac_184_each_subject_link_is_a_subject_edge_with_its_role() {
    let conversion = convert_value(&full_export()).unwrap();
    let subjects: Vec<&Edge> = edges(&conversion)
        .iter()
        .filter(|edge| matches!(edge, Edge::Subject { .. }))
        .collect();
    assert_eq!(
        subjects,
        [
            &Edge::Subject {
                decision: DecisionId::new(key(20)),
                subject: RequirementId::new(key(4)).into(),
                role: SubjectRole::Outcome,
            },
            &Edge::Subject {
                decision: DecisionId::new(key(20)),
                subject: ComponentId::new(key(6)).into(),
                role: SubjectRole::Affected,
            },
        ]
    );
}

#[test]
fn ac_244_every_decision_has_its_v1_external_id_as_label() {
    let conversion = convert_value(&full_export()).unwrap();
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
    let conversion = convert_value(&full_export()).unwrap();
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
    let conversion = convert_value(&full_export()).unwrap();
    let hashes = hash_document(&conversion.document).unwrap();
    assert_eq!(hashes.decisions.len(), 2);
}

#[test]
fn a_realization_link_whose_ends_are_not_a_feature_and_a_component_is_refused() {
    let mut value = full_export();
    value["realizationLinks"] = json!([{"featureId": uuid(4), "componentId": uuid(6)}]);
    let error = convert_value(&value).unwrap_err();
    assert!(matches!(error, ConvertError::Link { .. }), "{error}");
}

/// The `full_export()` with ADR-1's outcome subject and its subject driver naming nodes the
/// export no longer holds, last labelled RQ-9 and RQ-8.
fn with_deleted_subjects() -> serde_json::Value {
    let mut value = full_export();
    let decision = &mut value["architectureDecisions"][0];
    let link = &mut decision["subjectLinks"][0]["subject"];
    link["nodeId"] = json!(uuid(77));
    link["externalId"] = json!("RQ-9");
    let driver = &mut decision["elements"][4]["payload"]["value"]["subject"];
    driver["nodeId"] = json!(uuid(78));
    driver["externalId"] = json!("RQ-8");
    value
}

fn former(text: &str) -> NodeRef {
    NodeRef::Former(Label::parse(text).unwrap())
}

#[test]
fn ac_723_ac_708_and_ac_709_a_reference_to_a_node_the_export_does_not_hold_is_its_former_label() {
    let conversion = convert_value(&with_deleted_subjects()).unwrap();
    assert!(edges(&conversion).contains(&Edge::Subject {
        decision: DecisionId::new(key(20)),
        subject: former("RQ-9"),
        role: SubjectRole::Outcome,
    }));
    assert_eq!(
        conversion.document.decisions[0].drivers[1].payload,
        DriverPayload::Subject {
            subject: former("RQ-8")
        }
    );
}

#[test]
fn ac_711_and_ac_712_a_former_label_enters_the_edges_root_and_the_decision_view_as_an_object() {
    let conversion = convert_value(&with_deleted_subjects()).unwrap();
    let hashes = hash_document(&conversion.document).unwrap();
    let (_, _, edges_view) = hashes
        .root_views
        .iter()
        .find(|(name, _, _)| *name == "edges")
        .unwrap();
    assert!(edges_view.to_string().contains(r#"{"formerLabel":"RQ-9"}"#));
    let decision_view = &hashes.decision_views[&DecisionId::new(key(20))];
    assert!(
        decision_view
            .to_string()
            .contains(r#"{"formerLabel":"RQ-8"}"#)
    );
}

#[test]
fn a_subject_of_no_kind_the_format_holds_is_refused() {
    let mut value = full_export();
    value["architectureDecisions"][0]["subjectLinks"][0]["subject"]["type"] = json!("FacetTag");
    value["architectureDecisions"][0]["subjectLinks"][0]["subject"]["tier"] = json!(null);
    value["architectureDecisions"][0]["subjectLinks"][0]["subject"]["nodeId"] = json!(uuid(77));
    let error = convert_value(&value).unwrap_err();
    assert!(error.to_string().contains("ADR-1"), "{error}");
}

#[test]
fn ac_190_a_resolvable_reference_in_a_name_or_body_is_a_mentions_edge() {
    let mut value = full_export();
    value["nodes"][3]["body"] = json!("Builds on RQ-2, D-1/F-1 and RQ-2 again; see ADR-1.");
    let conversion = convert_value(&value).unwrap();
    let mentions: Vec<&Edge> = edges(&conversion)
        .iter()
        .filter(|edge| matches!(edge, Edge::Mentions { .. }))
        .collect();
    let source = ElementId::from(RequirementId::new(key(4)));
    assert_eq!(
        mentions,
        [
            &Edge::Mentions {
                source,
                target: ElementId::from(RequirementId::new(key(13))),
            },
            &Edge::Mentions {
                source,
                target: ElementId::from(FeatureId::new(key(3))),
            },
            &Edge::Mentions {
                source,
                target: ElementId::from(DecisionId::new(key(20))),
            },
        ]
    );
}

#[test]
fn ac_191_a_reference_that_does_not_resolve_is_a_finding() {
    let mut value = full_export();
    value["nodes"][4]["name"] = json!("Every node record carries the key RQ-99 names.");
    let conversion = convert_value(&value).unwrap();
    assert!(conversion.findings.contains(&Finding::UnresolvedReference {
        node: NodeId::Criterion(CriterionId::new(key(5))),
        reference: Label::parse("RQ-99").unwrap(),
    }));
}

#[test]
fn ac_192_and_ac_193_a_body_is_kept_byte_for_byte_and_a_heading_in_it_is_a_finding() {
    let body = "Keys.\n\n## Rationale\n\tTabbed, trailing space. \n";
    let mut value = full_export();
    value["nodes"][2]["body"] = json!(body);
    let conversion = convert_value(&value).unwrap();
    let feature = &conversion.document.products[0].feature_groups[0].features[0];
    assert_eq!(feature.details.body.as_deref(), Some(body));
    assert!(conversion.findings.contains(&Finding::HeadingInBody {
        node: NodeId::Feature(FeatureId::new(key(3))),
        line: 3,
    }));
}

/// The full export with each listed Component's v1 scope replaced.
fn scoped(mut export: serde_json::Value, scopes: &[(u32, serde_json::Value)]) -> serde_json::Value {
    for node in export["nodes"].as_array_mut().unwrap() {
        if let Some((_, scope)) = scopes.iter().find(|(n, _)| node["nodeId"] == uuid(*n)) {
            node["componentScope"] = scope.clone();
        }
    }
    export
}

fn boundary_of(conversion: &Conversion, n: u32) -> Option<Boundary> {
    let mut found = None;
    conversion.document.walk(&mut |node| {
        if let loftspec::node::Node::Component(component) = node
            && component.id == Some(ComponentId::new(key(n)))
        {
            found = Some(component.boundary);
        }
    });
    found.expect("the Component is converted")
}

#[test]
fn ac_892_a_component_the_export_scopes_as_external_has_the_boundary_external() {
    let export = scoped(full_export(), &[(6, json!({"kind": "external"}))]);
    let conversion = convert_value(&export).unwrap();
    assert_eq!(boundary_of(&conversion, 6), Some(Boundary::External));
    assert_eq!(
        boundary_of(&conversion, 10),
        None,
        "the nested Component takes it on"
    );
}

#[test]
fn ac_893_a_component_scoped_to_a_product_or_the_document_has_no_boundary() {
    let conversion = convert_value(&full_export()).unwrap();
    assert_eq!(boundary_of(&conversion, 6), None);
    let export = scoped(full_export(), &[(6, json!({"kind": "document"}))]);
    assert_eq!(boundary_of(&convert_value(&export).unwrap(), 6), None);
}

#[test]
fn ac_894_a_component_scoped_internally_beneath_an_external_one_is_a_finding() {
    let export = scoped(
        full_export(),
        &[
            (6, json!({"kind": "external"})),
            (10, json!({"kind": "document"})),
        ],
    );
    let conversion = convert_value(&export).unwrap();
    assert!(
        conversion
            .findings
            .contains(&Finding::InternalUnderExternal {
                component: ComponentId::new(key(10)),
                ancestor: ComponentId::new(key(6)),
            })
    );
    assert_eq!(boundary_of(&conversion, 10), None);
}

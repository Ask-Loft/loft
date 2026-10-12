//! The sidecar of a v1 conversion (D-11 F-15, RQ-55, RQ-58): every v1 fact the content view
//! excludes that the round trip to the source state needs, written deterministically.

mod common;

use common::v1::{convert_value, key, tree, uuid};
use loftspec::attributes::VerificationFate;
use loftspec::convert::v1::sidecar::{FORMAT, ScopeKind};
use loftspec::convert::v1::{Conversion, ConvertError};
use loftspec::identity::{Label, LabelPrefix};
use loftspec::ids::{
    ComponentId, ConstraintId, DecisionId, DocumentId, NodeId, OptionId, QualityAttributeId,
    RequirementId,
};
use serde_json::{Value, json};

fn full_export() -> Value {
    let mut value = common::v1::full_export();
    value["nodes"][9]["componentScope"] = json!({"kind": "external"});
    value["nodes"][6]["proposedFate"] = json!("proved");
    // A subject the export no longer holds, labelled above every live Requirement.
    let subject = &mut value["architectureDecisions"][0]["subjectLinks"][0]["subject"];
    subject["nodeId"] = json!(uuid(77));
    subject["externalId"] = json!("RQ-9");
    value
}

fn converted() -> Conversion {
    convert_value(&full_export()).unwrap()
}

#[test]
fn each_node_keeps_its_ordinal_confirmation_and_proposed_fate() {
    let sidecar = converted().sidecar;
    let constraint = sidecar.nodes[&NodeId::from(ConstraintId::new(key(7)))];
    assert_eq!(constraint.ordinal, 0);
    assert_eq!(constraint.fate_confirmed, Some(true));
    assert_eq!(constraint.proposed_fate, Some(VerificationFate::Proved));
    let text_requirement = sidecar.nodes[&NodeId::from(RequirementId::new(key(13)))];
    assert_eq!(text_requirement.ordinal, 1);
    let quality = sidecar.nodes[&NodeId::from(QualityAttributeId::new(key(8)))];
    assert_eq!(quality.fate_confirmed, None);
    assert_eq!(
        sidecar.nodes.len(),
        tree().len() - 2,
        "every node but the Facet Tags"
    );
}

#[test]
fn each_tag_application_keeps_its_facet_tag_node() {
    let sidecar = converted().sidecar;
    let [application] = sidecar.tags[&NodeId::from(ComponentId::new(key(6)))].as_slice() else {
        panic!("CMP-1 has one tag");
    };
    assert_eq!(application.tag.as_str(), "identity");
    assert_eq!(application.node_id.to_string(), uuid(14));
    assert_eq!(application.external_id.to_string(), "FT-2");
    assert_eq!(application.ordinal, 3);
}

#[test]
fn the_document_and_the_highest_value_of_each_prefix() {
    let sidecar = converted().sidecar;
    assert_eq!(sidecar.document, DocumentId::new(key(0)));
    let highest = |prefix: &str| sidecar.highest[prefix].get();
    assert_eq!(highest("RQ"), 9, "a former label counts");
    assert_eq!(highest("CMP"), 2);
    assert_eq!(highest("FT"), 2);
    assert_eq!(highest("ADR"), 2);
}

#[test]
fn a_former_label_counts_as_seen_so_it_is_never_allocated_again() {
    let counters = converted().document.counters;
    assert_eq!(counters.next(LabelPrefix::Requirement).get(), 10);
}

#[test]
fn each_component_keeps_its_v1_scope_kind() {
    let sidecar = converted().sidecar;
    assert_eq!(
        sidecar.scopes.get(&ComponentId::new(key(6))),
        Some(&ScopeKind::Product)
    );
    assert_eq!(
        sidecar.scopes.get(&ComponentId::new(key(10))),
        Some(&ScopeKind::External)
    );
    let mut unscoped = full_export();
    unscoped["nodes"][9]["componentScope"] = Value::Null;
    let sidecar = convert_value(&unscoped).unwrap().sidecar;
    assert_eq!(sidecar.scopes.get(&ComponentId::new(key(10))), None);
}

#[test]
fn decision_elements_keep_their_ordinals_and_a_former_label_its_v1_node() {
    let sidecar = converted().sidecar;
    let decision = &sidecar.decisions[&DecisionId::new(key(20))];
    assert_eq!(decision.ordinal, 0);
    assert_eq!(decision.options[&OptionId::new(key(24))], 1);
    assert_eq!(decision.drivers.len(), 2);
    assert_eq!(decision.consequences.len(), 2);
    let former = Label::parse("RQ-9").unwrap();
    assert_eq!(sidecar.former_labels[&former].to_string(), uuid(77));
}

#[test]
fn two_conversions_write_identical_sidecars() {
    let first = converted().sidecar.to_json();
    assert_eq!(first, converted().sidecar.to_json());
    assert!(first.ends_with("}\n"));
    let reread: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(reread["format"], FORMAT);
    assert_eq!(reread["nodes"][uuid(7)]["proposedFate"], "proved");
    assert_eq!(reread["tags"][uuid(6)][0]["externalId"], "FT-2");
}

#[test]
fn a_facet_tag_with_a_body_is_refused() {
    let mut value = full_export();
    value["nodes"][13]["body"] = json!("Keys and labels.");
    let error = convert_value(&value).unwrap_err();
    assert!(
        matches!(error, ConvertError::FacetTagBody { .. }),
        "{error}"
    );
}

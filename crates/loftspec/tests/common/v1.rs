//! Builders for small Loft v1 exports, as JSON, for the converter's tests. Every node is in
//! document `uuid(0)`; node `n` has UUID `uuid(n)` and node hash `hash(n)`.

use serde_json::{Value, json};

pub fn hash(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

pub fn uuid(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

pub fn node(n: u32, node_type: &str, tier: &Value, external_id: &str, children: &[u32]) -> Value {
    json!({
        "nodeId": uuid(n),
        "externalId": external_id,
        "type": node_type,
        "documentId": uuid(0),
        "name": format!("Node {n}"),
        "body": null,
        "tier": tier,
        "verificationFate": null,
        "conservationKind": null,
        "qaKind": null,
        "fateConfirmed": null,
        "proposedFate": null,
        "technicalBinding": null,
        "componentScope": null,
        "qualityTarget": null,
        "ordinal": 0,
        "childrenHashes": children
            .iter()
            .map(|child| json!({"nodeId": uuid(*child), "nodeHash": hash(u8::try_from(*child).unwrap())}))
            .collect::<Vec<_>>(),
        "nodeHash": hash(u8::try_from(n).unwrap()),
    })
}

pub fn export(nodes: &[Value], decisions: &[Value]) -> Value {
    json!({
        "annotations": [],
        "architectureDecisionAddress": hash(90),
        "architectureDecisions": decisions,
        "authoredStateId": uuid(99),
        "contentAddress": hash(91),
        "graphAddress": hash(92),
        "nodes": nodes,
        "publishSequence": 7,
        "publishedAt": "2026-10-11T15:10:44.298360465Z",
        "realizationLinks": [{"featureId": uuid(3), "componentId": uuid(6)}],
        "technicalConstraintCategories": [{
            "categoryEntryHash": hash(93),
            "id": uuid(70),
            "documentId": uuid(0),
            "name": "runtime.language",
            "definition": "The language the runtime is written in.",
        }],
        "technicalConstraintCategoryAddress": hash(94),
        "vocabularyAddress": hash(95),
        "vocabularyEntries": [{
            "entryHash": hash(96),
            "entryId": uuid(71),
            "documentId": uuid(0),
            "name": "portable",
            "definition": "Behaviour every implementation reproduces.",
        }],
    })
}

pub fn named(mut node: Value, name: &str, ordinal: i64) -> Value {
    node["name"] = json!(name);
    node["ordinal"] = json!(ordinal);
    node
}

pub fn tier(name: &str) -> Value {
    json!(name)
}

pub fn constraint(n: u32, label: &str) -> Value {
    let mut constraint = node(n, "Constraint", &Value::Null, label, &[]);
    constraint["verificationFate"] = json!("monitored");
    constraint["fateConfirmed"] = json!(true);
    constraint["technicalBinding"] =
        json!({"category": "runtime.language", "requiredValue": "rust"});
    named(constraint, &format!("Constraint {n} shall hold."), 0)
}

pub fn quality(n: u32, label: &str) -> Value {
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
pub fn tree() -> Vec<Value> {
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

pub fn with_identity_entry(mut value: Value) -> Value {
    value["vocabularyEntries"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "entryHash": "ab".repeat(32), "entryId": uuid(72), "documentId": uuid(0),
            "name": "identity", "definition": "Keys and labels.",
        }));
    value
}

pub fn subject(n: u32, external_id: &str) -> Value {
    json!({"externalId": external_id, "nodeId": uuid(n), "tier": "Requirement", "type": "TierSection"})
}

pub fn element(n: u32, parent: Option<u32>, ordinal: i64, payload: &Value) -> Value {
    json!({
        "documentId": uuid(0),
        "elementHash": hash(u8::try_from(n).unwrap()),
        "id": uuid(n),
        "ordinal": ordinal,
        "parentId": parent.map(uuid),
        "payload": payload,
    })
}

/// ADR-1, accepted: two options, a local driver and one naming RQ-1 (node 4), two consequences
/// (one naming RQ-1), four assessments, RQ-1 as outcome and CMP-1 (node 6) as affected. ADR-2,
/// superseded by ADR-1, with one option.
pub fn decisions() -> Vec<Value> {
    let component =
        json!({"externalId": "CMP-1", "nodeId": uuid(6), "tier": null, "type": "Component"});
    let assessment = |option: u32, driver: u32, rating: &str| {
        json!({"decisionId": uuid(20), "optionId": uuid(option), "driverId": uuid(driver),
               "rating": rating, "rationale": format!("{rating} {option} {driver}")})
    };
    let adr_1 = json!({
        "identity": {"decisionId": uuid(20), "documentId": uuid(0), "externalId": "ADR-1"},
        "decisionHash": hash(20),
        "elements": [
            element(20, None, 0, &json!({"kind": "architecture-decision", "value": {
                "title": "Keys are UUIDs", "context": "Every node carries its key.",
                "state": {"kind": "accepted", "selectedOptionId": uuid(23), "outcome": "Version 7."},
            }})),
            element(24, Some(20), 1, &json!({"kind": "option", "value": {"title": "ULID", "description": "Base32."}})),
            element(23, Some(20), 0, &json!({"kind": "option", "value": {"title": "UUID", "description": "RFC 9562."}})),
            element(21, Some(20), 0, &json!({"kind": "driver", "value": {"kind": "local", "label": "D1", "statement": "One shape."}})),
            element(22, Some(20), 1, &json!({"kind": "driver", "value": {"kind": "specification-subject", "subject": subject(4, "RQ-1")}})),
            element(25, Some(20), 0, &json!({"kind": "consequence", "value": {"kind": "obligation", "statement": "Mint keys.", "subject": subject(4, "RQ-1")}})),
            element(26, Some(20), 1, &json!({"kind": "consequence", "value": {"kind": "risk", "statement": "A key reveals time.", "subject": null}})),
        ],
        "assessments": [
            assessment(23, 21, "supports"), assessment(23, 22, "supports"),
            assessment(24, 21, "conflicts"), assessment(24, 22, "unknown"),
        ],
        "subjectLinks": [
            {"decisionId": uuid(20), "role": "outcome", "subject": subject(4, "RQ-1")},
            {"decisionId": uuid(20), "role": "affected", "subject": component},
        ],
    });
    let adr_2 = json!({
        "identity": {"decisionId": uuid(30), "documentId": uuid(0), "externalId": "ADR-2"},
        "decisionHash": hash(30),
        "elements": [
            element(30, None, 0, &json!({"kind": "architecture-decision", "value": {
                "title": "Keys are integers", "context": "An earlier choice.",
                "state": {"kind": "superseded", "selectedOptionId": uuid(31), "outcome": "Integers.",
                          "successorDecisionId": uuid(20)},
            }})),
            element(31, Some(30), 0, &json!({"kind": "option", "value": {"title": "Integers", "description": "Counters."}})),
        ],
        "assessments": [],
        "subjectLinks": [],
    });
    vec![adr_1, adr_2]
}

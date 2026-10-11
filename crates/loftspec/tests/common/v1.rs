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

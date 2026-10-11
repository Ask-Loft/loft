//! The small document the hashing tests and the golden vectors share. `testdata/golden/README.md`
//! describes the same document in prose.

#![allow(dead_code)]

use loftspec::attributes::{
    Attributes, QaKind, QualityAggregation, QualityBand, QualityTarget, TechnicalBinding,
    VerificationFate,
};
use loftspec::decision::{
    Assessment, Consequence, ConsequenceKind, Decision, DecisionOption, DecisionState, Driver,
    DriverPayload, Rating,
};
use loftspec::identity::{Key, Label};
use loftspec::kind::NodeKind;
use loftspec::model::{Document, Edge, EdgeKind, Node, RegistryEntry, SubjectRole, Title};
use loftspec::statement::{EarsForm, Statement};

/// A deterministic version 7 key numbered `n`.
pub fn key(n: u32) -> Key {
    Key::parse(&format!("00000000-0000-7000-8000-{n:012x}")).unwrap()
}

pub fn label(text: &str) -> Label {
    Label::parse(text).unwrap()
}

pub fn named(n: u32, kind: NodeKind, name: &str) -> Node {
    Node::named(Some(key(n)), kind, name)
}

fn requirement() -> Node {
    let mut node = named(4, NodeKind::Requirement, "");
    node.title = Title::Statement(Statement::Form(EarsForm::Ubiquitous {
        system: "The writer".to_owned(),
        shall: "write a key on every node.".to_owned(),
    }));
    node.children = vec![named(
        5,
        NodeKind::Criterion,
        "Every node record carries a `key` field.",
    )];
    node
}

fn product() -> Node {
    let mut feature = named(3, NodeKind::Feature, "Keys");
    feature.body = Some("Keys address nodes.".to_owned());
    feature.children = vec![requirement()];
    let mut group = named(2, NodeKind::FeatureGroup, "Identity");
    group.children = vec![feature];
    let mut product = named(1, NodeKind::Product, "Example Product");
    product.body = Some("The example.".to_owned());
    product.tags = vec!["portable".to_owned()];
    product.children = vec![group];
    product
}

fn component() -> Node {
    let mut constraint = named(
        7,
        NodeKind::Constraint,
        "The document model shall reject unknown kinds.",
    );
    constraint.attributes = Attributes {
        constraint_binding: Some(TechnicalBinding {
            category: "runtime.language".to_owned(),
            required_value: "rust".to_owned(),
        }),
        verification_fate: Some(VerificationFate::Proved),
        ..Attributes::default()
    };
    let mut quality = named(
        8,
        NodeKind::QualityAttribute,
        "Formatting completes quickly.",
    );
    quality.attributes = Attributes {
        qa_kind: Some(QaKind::Latency),
        quality_target: Some(QualityTarget {
            metric: "fmt.wall-time".to_owned(),
            band: QualityBand::AtMost {
                upper: "2".to_owned(),
            },
            unit: "s".to_owned(),
            aggregation: QualityAggregation::P95,
            scenario: "formatting a canonical tree of five thousand nodes".to_owned(),
            environment: "the reference developer machine".to_owned(),
        }),
        verification_fate: Some(VerificationFate::Sampled),
        ..Attributes::default()
    };
    let mut component = named(6, NodeKind::Component, "Document model");
    component.body =
        Some("Nodes and keys.\n\tAn indented line with a trailing space. \n".to_owned());
    component.children = vec![constraint, quality];
    component
}

fn decision() -> Decision {
    Decision {
        key: key(20),
        title: "Keys are UUIDs".to_owned(),
        context: "Every node carries its key in the file.".to_owned(),
        state: DecisionState::Accepted {
            selected_option: key(23),
            outcome: "New keys are UUID version 7.".to_owned(),
        },
        drivers: vec![
            Driver {
                key: key(21),
                payload: DriverPayload::Local {
                    label: "One key shape".to_owned(),
                    statement: "Converted and new keys share one syntax.".to_owned(),
                },
            },
            Driver {
                key: key(22),
                payload: DriverPayload::Subject { subject: key(4) },
            },
        ],
        options: vec![
            DecisionOption {
                key: key(23),
                title: "UUID version 7".to_owned(),
                description: "Time-ordered RFC 9562 UUIDs.".to_owned(),
            },
            DecisionOption {
                key: key(24),
                title: "ULID".to_owned(),
                description: "Crockford base32 identifiers.".to_owned(),
            },
        ],
        assessments: vec![
            assessment(24, 22, Rating::Neutral, "Either can be written."),
            assessment(
                23,
                21,
                Rating::Supports,
                "The same syntax as converted keys.",
            ),
            assessment(24, 21, Rating::Conflicts, "Two syntaxes in one document."),
            assessment(23, 22, Rating::Supports, "The writer writes one shape."),
        ],
        consequences: vec![
            Consequence {
                key: key(25),
                kind: ConsequenceKind::Obligation,
                statement: "The formatter mints version 7 keys.".to_owned(),
                subject: Some(key(4)),
            },
            Consequence {
                key: key(26),
                kind: ConsequenceKind::Risk,
                statement: "A key reveals when it was minted.".to_owned(),
                subject: None,
            },
        ],
    }
}

fn assessment(option: u32, driver: u32, rating: Rating, rationale: &str) -> Assessment {
    Assessment {
        option: key(option),
        driver: key(driver),
        rating,
        rationale: rationale.to_owned(),
    }
}

fn entry(n: u32, name: &str, definition: &str) -> RegistryEntry {
    RegistryEntry {
        key: key(n),
        name: name.to_owned(),
        definition: definition.to_owned(),
    }
}

pub fn edge(source: u32, kind: EdgeKind, target: u32, role: Option<SubjectRole>) -> Edge {
    Edge {
        source: key(source),
        kind,
        target: key(target),
        role,
    }
}

/// The small document: P-1 > FG-1 > F-1 > RQ-1 > one criterion, root Component CMP-1 with CON-1
/// and QA-1, five edges, two tags, one constraint category and ADR-1.
pub fn small_document() -> Document {
    let mut document = Document::empty(key(0));
    document.products = vec![product()];
    document.components = vec![component()];
    document.edges = vec![
        edge(3, EdgeKind::Realizes, 6, None),
        edge(6, EdgeKind::ScopedTo, 1, None),
        edge(5, EdgeKind::Mentions, 4, None),
        edge(20, EdgeKind::Subject, 4, Some(SubjectRole::Outcome)),
        edge(20, EdgeKind::Subject, 6, Some(SubjectRole::Affected)),
    ];
    document.tags = vec![
        entry(
            31,
            "portable",
            "Behaviour every implementation must reproduce exactly.",
        ),
        entry(
            30,
            "identity",
            "Keys, labels, aliases and the bindings between them.",
        ),
    ];
    document.categories = vec![entry(
        32,
        "runtime.language",
        "The language the runtime is written in.",
    )];
    document.decisions = vec![decision()];
    for (n, text) in [
        (1, "P-1"),
        (2, "FG-1"),
        (3, "F-1"),
        (6, "CMP-1"),
        (7, "CON-1"),
        (8, "QA-1"),
        (20, "ADR-1"),
    ] {
        document.bindings.bind(key(n), label(text)).unwrap();
    }
    document.bindings.bind(key(4), label("RQ-1")).unwrap();
    document.bindings.relabel(key(4), label("RQ-2")).unwrap();
    document
}

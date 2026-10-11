//! The small document the hashing tests and the golden vectors share. `testdata/golden/README.md`
//! describes the same document in prose.

#![allow(dead_code)]

use loftspec::attributes::{
    QaKind, QualityAggregation, QualityBand, QualityTarget, TechnicalBinding, VerificationFate,
};
use loftspec::decision::{
    Assessment, Consequence, ConsequenceKind, Decision, DecisionOption, DecisionState, Driver,
    DriverPayload, Rating,
};
use loftspec::document::{CategoryEntry, Document, Edge, SubjectRole, TagEntry};
use loftspec::identity::{Key, Label};
use loftspec::ids::{
    CategoryId, ComponentId, ConsequenceId, ConstraintId, CriterionId, DecisionId, DocumentId,
    DriverId, FeatureGroupId, FeatureId, OptionId, ProductId, QualityAttributeId, RequirementId,
    TagId,
};
use loftspec::names::{CategoryName, TagName};
use loftspec::statement::{EarsForm, Statement};
use loftspec::tree::{
    Component, Constraint, Criterion, Details, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement,
};

/// A deterministic version 7 key numbered `n`.
pub fn key(n: u32) -> Key {
    Key::parse(&format!("00000000-0000-7000-8000-{n:012x}")).unwrap()
}

/// A different deterministic key for every `n`, to build the same document with fresh keys.
pub fn fresh_key(n: u32) -> Key {
    key(n + 0x1000)
}

pub fn label(text: &str) -> Label {
    Label::parse(text).unwrap()
}

pub fn criterion(id: Key, name: &str) -> Criterion {
    Criterion {
        id: Some(CriterionId::new(id)),
        name: name.to_owned(),
        details: Details::default(),
    }
}

pub fn requirement(id: Key, text: &str, criteria: Vec<Criterion>) -> Requirement {
    Requirement {
        id: Some(RequirementId::new(id)),
        statement: Statement::Text(text.to_owned()),
        details: Details::default(),
        criteria,
    }
}

pub fn feature(id: Key, name: &str, requirements: Vec<Requirement>) -> Feature {
    Feature {
        id: Some(FeatureId::new(id)),
        name: name.to_owned(),
        details: Details::default(),
        requirements,
        quality_attributes: Vec::new(),
    }
}

pub fn component(id: Key, name: &str) -> Component {
    Component {
        id: Some(ComponentId::new(id)),
        name: name.to_owned(),
        details: Details::default(),
        components: Vec::new(),
        constraints: Vec::new(),
        quality_attributes: Vec::new(),
    }
}

pub fn product(id: Key, name: &str) -> Product {
    Product {
        id: Some(ProductId::new(id)),
        name: name.to_owned(),
        details: Details::default(),
        feature_groups: Vec::new(),
    }
}

fn product_tree(k: &dyn Fn(u32) -> Key) -> Product {
    let mut requirement = requirement(
        k(4),
        "",
        vec![criterion(k(5), "Every node record carries a `key` field.")],
    );
    requirement.statement = Statement::Form(EarsForm::Ubiquitous {
        system: "The writer".to_owned(),
        shall: "write a key on every node.".to_owned(),
    });
    let mut feature = feature(k(3), "Keys", vec![requirement]);
    feature.details = Details::with_body("Keys address nodes.");
    feature.quality_attributes = vec![QualityAttribute {
        id: Some(QualityAttributeId::new(k(10))),
        name: "Minting a key completes quickly.".to_owned(),
        details: Details::default(),
        kind: QaKind::Latency,
        target: QualityTarget {
            metric: "mint.wall-time".to_owned(),
            band: QualityBand::AtMost {
                upper: "1".to_owned(),
            },
            unit: "ms".to_owned(),
            aggregation: QualityAggregation::P99,
            scenario: "minting one key".to_owned(),
            environment: "the reference developer machine".to_owned(),
        },
        verification_fate: Some(VerificationFate::Sampled),
    }];
    let group = FeatureGroup {
        id: Some(FeatureGroupId::new(k(2))),
        name: "Identity".to_owned(),
        details: Details::default(),
        features: vec![feature],
        constraints: vec![Constraint {
            id: Some(ConstraintId::new(k(9))),
            name: "Every Product shall follow the identity rules.".to_owned(),
            details: Details::default(),
            binding: None,
            conservation_kind: None,
            verification_fate: Some(VerificationFate::Proved),
        }],
    };
    let mut product = product(k(1), "Example Product");
    product.details = Details {
        tags: vec![TagName::parse("portable").unwrap()],
        ..Details::with_body("The example.")
    };
    product.feature_groups = vec![group];
    product
}

fn component_tree(k: &dyn Fn(u32) -> Key) -> Component {
    let constraint = Constraint {
        id: Some(ConstraintId::new(k(7))),
        name: "The document model shall reject unknown kinds.".to_owned(),
        details: Details::default(),
        binding: Some(TechnicalBinding {
            category: CategoryName::parse("runtime.language").unwrap(),
            required_value: "rust".to_owned(),
        }),
        conservation_kind: None,
        verification_fate: Some(VerificationFate::Proved),
    };
    let quality = QualityAttribute {
        id: Some(QualityAttributeId::new(k(8))),
        name: "Formatting completes quickly.".to_owned(),
        details: Details::default(),
        kind: QaKind::Latency,
        target: QualityTarget {
            metric: "fmt.wall-time".to_owned(),
            band: QualityBand::AtMost {
                upper: "2".to_owned(),
            },
            unit: "s".to_owned(),
            aggregation: QualityAggregation::P95,
            scenario: "formatting a canonical tree of five thousand nodes".to_owned(),
            environment: "the reference developer machine".to_owned(),
        },
        verification_fate: Some(VerificationFate::Sampled),
    };
    let mut component = component(k(6), "Document model");
    component.details =
        Details::with_body("Nodes and keys.\n\tAn indented line with a trailing space. \n");
    component.constraints = vec![constraint];
    component.quality_attributes = vec![quality];
    component
}

fn decision(k: &dyn Fn(u32) -> Key) -> Decision {
    let assessment = |option: u32, driver: u32, rating: Rating, rationale: &str| Assessment {
        option: OptionId::new(k(option)),
        driver: DriverId::new(k(driver)),
        rating,
        rationale: rationale.to_owned(),
    };
    Decision {
        id: DecisionId::new(k(20)),
        title: "Keys are UUIDs".to_owned(),
        context: "Every node carries its key in the file.".to_owned(),
        state: DecisionState::Accepted {
            selected_option: OptionId::new(k(23)),
            outcome: "New keys are UUID version 7.".to_owned(),
        },
        drivers: vec![
            Driver {
                id: DriverId::new(k(21)),
                payload: DriverPayload::Local {
                    label: "One key shape".to_owned(),
                    statement: "Converted and new keys share one syntax.".to_owned(),
                },
            },
            Driver {
                id: DriverId::new(k(22)),
                payload: DriverPayload::Subject {
                    subject: RequirementId::new(k(4)).into(),
                },
            },
        ],
        options: vec![
            DecisionOption {
                id: OptionId::new(k(23)),
                title: "UUID version 7".to_owned(),
                description: "Time-ordered RFC 9562 UUIDs.".to_owned(),
            },
            DecisionOption {
                id: OptionId::new(k(24)),
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
                id: ConsequenceId::new(k(25)),
                kind: ConsequenceKind::Obligation,
                statement: "The formatter mints version 7 keys.".to_owned(),
                subject: Some(RequirementId::new(k(4)).into()),
            },
            Consequence {
                id: ConsequenceId::new(k(26)),
                kind: ConsequenceKind::Risk,
                statement: "A key reveals when it was minted.".to_owned(),
                subject: None,
            },
        ],
    }
}

fn edges(k: &dyn Fn(u32) -> Key) -> Vec<Edge> {
    let decision = DecisionId::new(k(20));
    vec![
        Edge::Realizes {
            feature: FeatureId::new(k(3)),
            component: ComponentId::new(k(6)),
        },
        Edge::ScopedTo {
            component: ComponentId::new(k(6)),
            product: ProductId::new(k(1)),
        },
        Edge::Mentions {
            source: CriterionId::new(k(5)).into(),
            target: RequirementId::new(k(4)).into(),
        },
        Edge::Subject {
            decision,
            subject: RequirementId::new(k(4)).into(),
            role: SubjectRole::Outcome,
        },
        Edge::Subject {
            decision,
            subject: ComponentId::new(k(6)).into(),
            role: SubjectRole::Affected,
        },
    ]
}

fn bind(document: &mut Document, k: &dyn Fn(u32) -> Key) {
    let bindings = &mut document.bindings;
    bindings.bind(ProductId::new(k(1)), label("P-1")).unwrap();
    bindings
        .bind(FeatureGroupId::new(k(2)), label("FG-1"))
        .unwrap();
    bindings.bind(FeatureId::new(k(3)), label("F-1")).unwrap();
    bindings
        .bind(RequirementId::new(k(4)), label("RQ-1"))
        .unwrap();
    bindings
        .relabel(RequirementId::new(k(4)), label("RQ-2"))
        .unwrap();
    bindings
        .bind(ComponentId::new(k(6)), label("CMP-1"))
        .unwrap();
    bindings
        .bind(ConstraintId::new(k(7)), label("CON-1"))
        .unwrap();
    bindings
        .bind(QualityAttributeId::new(k(8)), label("QA-1"))
        .unwrap();
    bindings
        .bind(ConstraintId::new(k(9)), label("CON-2"))
        .unwrap();
    bindings
        .bind(QualityAttributeId::new(k(10)), label("QA-2"))
        .unwrap();
    bindings
        .bind(DecisionId::new(k(20)), label("ADR-1"))
        .unwrap();
}

/// The small document built with the keys `k` gives: P-1 > FG-1 > F-1 > RQ-2 (alias RQ-1) > one
/// unlabelled criterion, with QA-2 under F-1 and CON-2 under FG-1; root Component CMP-1 with
/// CON-1 and QA-1; five edges, two tags, one constraint category and ADR-1.
pub fn small_document_with(k: &dyn Fn(u32) -> Key) -> Document {
    let mut document = Document::empty(DocumentId::new(k(0)));
    document.products = vec![product_tree(k)];
    document.components = vec![component_tree(k)];
    document.edges = edges(k);
    document.tags = vec![
        TagEntry {
            id: TagId::new(k(31)),
            name: TagName::parse("portable").unwrap(),
            definition: "Behaviour every implementation must reproduce exactly.".to_owned(),
        },
        TagEntry {
            id: TagId::new(k(30)),
            name: TagName::parse("identity").unwrap(),
            definition: "Keys, labels, aliases and the bindings between them.".to_owned(),
        },
    ];
    document.categories = vec![CategoryEntry {
        id: CategoryId::new(k(32)),
        name: CategoryName::parse("runtime.language").unwrap(),
        definition: "The language the runtime is written in.".to_owned(),
    }];
    document.decisions = vec![decision(k)];
    bind(&mut document, k);
    document
}

pub fn small_document() -> Document {
    small_document_with(&key)
}

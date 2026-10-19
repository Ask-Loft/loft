//! The kinds of the converter's refusals and of an export's errors: one name for each kind of
//! error, the same however its message is filled, so that a report can count errors by kind.
//! The reader's problems are held to the same in `src/codec/error.rs`, where their type can be
//! matched without a wildcard.

use std::collections::BTreeSet;

use loftspec::bindings::BindingError;
use loftspec::convert::v1::ConvertError;
use loftspec::convert::v1::export::{Export, ExternalId};
use loftspec::identity::LabelError;
use loftspec::ids::{ElementId, ProductId};
use loftspec::names::TagName;
use uuid::Uuid;

use crate::common::key;

fn id(text: &str) -> ExternalId {
    ExternalId::parse(text).unwrap()
}

fn uuid(n: u32) -> Uuid {
    key(n).uuid()
}

/// The place of each variant among [`every_refusal`]'s samples: a match with no wildcard, so
/// that a variant added to the type does not compile here until it is sampled.
const fn place(refusal: &ConvertError) -> usize {
    match refusal {
        ConvertError::NoDocument => 0,
        ConvertError::TwoDocuments { .. } => 1,
        ConvertError::OtherDocument { .. } => 2,
        ConvertError::Annotations => 3,
        ConvertError::UnknownChild { .. } => 4,
        ConvertError::TwoParents { .. } => 5,
        ConvertError::Orphan { .. } => 6,
        ConvertError::Containment { .. } => 7,
        ConvertError::Missing { .. } => 8,
        ConvertError::Label { .. } => 9,
        ConvertError::FacetTagBody { .. } => 10,
        ConvertError::UnderivableTag { .. } => 11,
        ConvertError::TagCollision { .. } => 12,
        ConvertError::Link { .. } => 13,
        ConvertError::Scope { .. } => 14,
        ConvertError::DecisionSubject { .. } => 15,
        ConvertError::DecisionElement { .. } => 16,
        ConvertError::Binding(_) => 17,
    }
}

/// A refusal of each kind, its values numbered `n`.
fn every_refusal(n: u32) -> Vec<ConvertError> {
    let node = id(&format!("RQ-{n}"));
    let other = id(&format!("F-{n}"));
    let element = ElementId::Node(ProductId::new(key(n)).into());
    vec![
        ConvertError::NoDocument,
        ConvertError::TwoDocuments {
            first: uuid(n),
            second: uuid(2),
        },
        ConvertError::OtherDocument {
            carried: uuid(n),
            named: uuid(2),
        },
        ConvertError::Annotations,
        ConvertError::UnknownChild {
            parent: other.clone(),
            child: uuid(n),
        },
        ConvertError::TwoParents {
            child: node.clone(),
            first: other.clone(),
            second: id("FG-1"),
        },
        ConvertError::Orphan { node: node.clone() },
        ConvertError::Containment {
            parent: other.clone(),
            child: node.clone(),
        },
        ConvertError::Missing {
            node: node.clone(),
            field: "qualityTarget",
        },
        ConvertError::Label {
            node: node.clone(),
            source: LabelError::Malformed(format!("X{n}")),
        },
        ConvertError::FacetTagBody { node: node.clone() },
        ConvertError::UnderivableTag {
            name: format!("{n}"),
        },
        ConvertError::TagCollision {
            first: format!("tag {n}"),
            second: format!("Tag {n}"),
            tag: TagName::parse(&format!("tag-{n}")).unwrap(),
        },
        ConvertError::Link {
            feature: uuid(n),
            component: uuid(2),
        },
        ConvertError::Scope {
            node: node.clone(),
            product: uuid(n),
        },
        ConvertError::DecisionSubject {
            decision: id(&format!("ADR-{n}")),
            subject: uuid(n),
        },
        ConvertError::DecisionElement {
            decision: id(&format!("ADR-{n}")),
            element: uuid(n),
            reason: "is not an option",
        },
        ConvertError::Binding(BindingError::Unbound(element)),
    ]
}

/// The kinds of `kinds`, each once, and how many there were.
fn distinct(kinds: &[&'static str]) -> (usize, BTreeSet<&'static str>) {
    (kinds.len(), kinds.iter().copied().collect())
}

#[test]
fn every_kind_of_refusal_is_sampled_once_in_order() {
    let places: Vec<usize> = every_refusal(1).iter().map(place).collect();
    assert_eq!(places, (0..places.len()).collect::<Vec<_>>());
}

#[test]
fn each_kind_of_refusal_has_a_kind_name_of_its_own() {
    let kinds: Vec<&str> = every_refusal(1).iter().map(ConvertError::kind).collect();
    let (count, set) = distinct(&kinds);
    assert_eq!(set.len(), count, "{kinds:?}");
}

#[test]
fn a_refusals_kind_name_does_not_depend_on_the_values_its_message_quotes() {
    for (one, other) in every_refusal(1).iter().zip(every_refusal(7)) {
        assert_eq!(one.kind(), other.kind());
    }
    let quoted = every_refusal(1)
        .iter()
        .zip(every_refusal(7))
        .filter(|(one, other)| one.to_string() != other.to_string())
        .count();
    assert!(quoted > 10, "the samples hardly differ: {quoted}");
}

#[test]
fn an_export_that_is_not_a_v1_export_is_named_by_what_is_wrong_with_its_text() {
    let kind = |text: &str| Export::from_json(text).unwrap_err().kind();
    assert_eq!(kind("not json"), "syntax");
    assert_eq!(kind("{\"nodes\": ["), "eof");
    assert_eq!(kind("{\"nodes\": []}"), "data");
}

//! The kinds of the converter's refusals and of the reader's problems: one name for each kind of
//! error, the same however its message is filled, so that a report can count errors by kind.

use std::collections::BTreeSet;

use loftspec::bindings::BindingError;
use loftspec::codec::Problem;
use loftspec::convert::v1::ConvertError;
use loftspec::convert::v1::export::{Export, ExternalId};
use loftspec::identity::{Key, LabelError};
use loftspec::ids::{ElementId, ProductId};
use loftspec::manifest::ManifestError;
use loftspec::names::TagName;
use loftspec::yaml::ErrorKind;
use uuid::Uuid;

fn id(text: &str) -> ExternalId {
    ExternalId::parse(text).unwrap()
}

fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn key(n: u128) -> Key {
    Key::from(uuid(n))
}

/// A refusal of each kind, its values numbered `n`.
fn every_refusal(n: u32) -> Vec<ConvertError> {
    let node = id(&format!("RQ-{n}"));
    let other = id(&format!("F-{n}"));
    let element = ElementId::Node(ProductId::new(key(n.into())).into());
    vec![
        ConvertError::NoDocument,
        ConvertError::TwoDocuments {
            first: uuid(n.into()),
            second: uuid(2),
        },
        ConvertError::OtherDocument {
            carried: uuid(n.into()),
            named: uuid(2),
        },
        ConvertError::Annotations,
        ConvertError::UnknownChild {
            parent: other.clone(),
            child: uuid(n.into()),
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
            feature: uuid(n.into()),
            component: uuid(2),
        },
        ConvertError::Scope {
            node: node.clone(),
            product: uuid(n.into()),
        },
        ConvertError::DecisionSubject {
            decision: id(&format!("ADR-{n}")),
            subject: uuid(n.into()),
        },
        ConvertError::DecisionElement {
            decision: id(&format!("ADR-{n}")),
            element: uuid(n.into()),
            reason: "is not an option",
        },
        ConvertError::Binding(BindingError::Unbound(element)),
    ]
}

/// A problem of each kind, its values numbered `n`.
fn every_problem(n: u32) -> Vec<Problem> {
    let text = format!("value-{n}");
    vec![
        Problem::Syntax(ErrorKind::NotPrintable(n)),
        Problem::UnknownField(text.clone()),
        Problem::MissingField("key"),
        Problem::Expected("a list"),
        Problem::Invalid {
            value: text.clone(),
            expected: "a key",
        },
        Problem::Shape(text.clone()),
        Problem::UnknownFile,
        Problem::MissingManifest,
        Problem::Unsupported(ManifestError::UnknownFormat(text.clone())),
        Problem::SlugMismatch {
            slug: text.clone(),
            name: format!("Value {n}"),
        },
        Problem::DuplicateKey(key(n.into())),
        Problem::MovedAndChanged(key(n.into())),
        Problem::ListedWithoutFile(key(n.into())),
        Problem::NotListed("Feature Group"),
        Problem::ListedTwice(key(n.into())),
        Problem::ListedByTwoGroups {
            feature: key(n.into()),
            first: text.clone(),
            second: "other".to_owned(),
        },
        Problem::WrongDirectory(text.clone()),
        Problem::Model(text.clone()),
        Problem::ByName,
        Problem::NameUnresolved {
            name: text.clone(),
            wanted: "decision",
        },
        Problem::NameAmbiguous {
            name: text,
            keys: vec![key(n.into()).to_string()],
        },
    ]
}

/// The kinds of `kinds`, each once, and how many there were.
fn distinct(kinds: &[&'static str]) -> (usize, BTreeSet<&'static str>) {
    (kinds.len(), kinds.iter().copied().collect())
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
fn each_kind_of_problem_has_a_kind_name_of_its_own() {
    let kinds: Vec<&str> = every_problem(1).iter().map(Problem::kind).collect();
    let (count, set) = distinct(&kinds);
    assert_eq!(set.len(), count, "{kinds:?}");
}

#[test]
fn a_problems_kind_name_does_not_depend_on_the_values_its_message_quotes() {
    for (one, other) in every_problem(1).iter().zip(every_problem(7)) {
        assert_eq!(one.kind(), other.kind());
    }
    let quoted = every_problem(1)
        .iter()
        .zip(every_problem(7))
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

//! Citations of withdrawn labels in lint (F-21): `V2-003` says that the next publish
//! rewrites a citation of a label withdrawn from a key the element mentions, and `V2-004` reports
//! a citation the publish cannot rewrite because the element also mentions the label's holder.

use crate::common::{key, label, small_document};
use loftspec::codec::Sites;
use loftspec::document::{Document, Edge};
use loftspec::ids::{ComponentId, FeatureId, RequirementId};
use loftspec::lint::{Finding, LintConfig, RuleId, Severity, lint};

/// The small document's Feature, whose body is `body`, mentioning the Requirement from which
/// `RQ-9` was withdrawn.
fn citing(body: &str) -> Document {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some(body.to_owned());
    document
        .bindings
        .add_withdrawn(RequirementId::new(key(4)), label("RQ-9"))
        .unwrap();
    document.edges.push(Edge::Mentions {
        source: FeatureId::new(key(3)).into(),
        target: RequirementId::new(key(4)).into(),
    });
    document
}

fn of(document: &Document, rule: RuleId) -> Vec<Finding> {
    lint(document, &Sites::default(), &LintConfig::default())
        .into_iter()
        .filter(|finding| finding.rule == rule)
        .collect()
}

#[test]
fn a_citation_of_a_label_withdrawn_from_a_mentioned_key_is_v2_003_saying_the_next_publish_rewrites_it()
 {
    let found = of(&citing("See RQ-9."), RuleId::V2003);
    let [finding] = found.as_slice() else {
        panic!("one finding: {found:?}");
    };
    assert_eq!(finding.term.as_deref(), Some("RQ-9"));
    assert_eq!(finding.subjects[0].key, key(3));
    assert_eq!(
        finding.message,
        "The label is withdrawn from a mentioned element; the next publish rewrites it."
    );
    assert_eq!(of(&citing("See RQ-9."), RuleId::V2004), Vec::new());
}

/// `RQ-9` held as an alias by the Component, which the Feature also mentions.
fn held_and_mentioned(body: &str) -> Document {
    let mut document = citing(body);
    document
        .bindings
        .add_alias(ComponentId::new(key(6)), label("RQ-9"))
        .unwrap();
    document.edges.push(Edge::Mentions {
        source: FeatureId::new(key(3)).into(),
        target: ComponentId::new(key(6)).into(),
    });
    document
}

#[test]
fn a_citation_in_an_element_that_mentions_the_withdrawing_key_and_the_holder_is_v2_004() {
    let document = held_and_mentioned("See RQ-9.");
    let found = of(&document, RuleId::V2004);
    let [finding] = found.as_slice() else {
        panic!("one finding: {found:?}");
    };
    assert_eq!(finding.term.as_deref(), Some("RQ-9"));
    assert_eq!(finding.subjects[0].key, key(3));
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(of(&document, RuleId::V2003), Vec::new());
}

#[test]
fn a_citation_in_an_element_that_mentions_only_the_holder_is_not_v2_004() {
    let mut document = held_and_mentioned("See RQ-9.");
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Mentions { target, .. } if target.key() == key(4)));
    assert_eq!(of(&document, RuleId::V2004), Vec::new());
    assert_eq!(of(&document, RuleId::V2003), Vec::new());
}

#[test]
fn v2_004_is_a_rule_with_a_wire_name_and_a_warning_by_default() {
    assert_eq!(RuleId::parse("V2-004"), Some(RuleId::V2004));
    assert_eq!(RuleId::V2004.default_severity(), Severity::Warning);
}

#[test]
fn a_citation_of_a_label_withdrawn_from_two_mentioned_keys_is_v2_004() {
    let mut document = citing("See RQ-9.");
    document
        .bindings
        .add_withdrawn(ComponentId::new(key(6)), label("RQ-9"))
        .unwrap();
    document.edges.push(Edge::Mentions {
        source: FeatureId::new(key(3)).into(),
        target: ComponentId::new(key(6)).into(),
    });
    let found = of(&document, RuleId::V2004);
    let [finding] = found.as_slice() else {
        panic!("one finding: {found:?}");
    };
    assert_eq!(
        finding.message,
        "The label is withdrawn from two mentioned elements; the next publish cannot rewrite it."
    );
    assert_eq!(of(&document, RuleId::V2003), Vec::new());
}

#[test]
fn a_qualified_citation_of_a_withdrawn_label_is_not_said_to_be_rewritten() {
    let found = of(&citing("See D-7/RQ-9."), RuleId::V2003);
    assert!(
        found
            .iter()
            .all(|finding| !finding.message.contains("rewrites it")),
        "{found:?}"
    );
}

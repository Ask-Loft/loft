//! The rules over the term list: `SH-011`, a name or alias two terms carry, and `LQ-010`, an
//! alias in prose (F-19, F-20).

use crate::common::lint::{added, findings, keys, of, severity};
use crate::common::{criterion, key, requirement, small_document, term};
use loftspec::document::{Document, TermEntry};
use loftspec::lint::{RuleId, Severity};

/// The small document with the term "record" (alias "entry") and one more Requirement under
/// F-1 whose statement is `text` and whose criterion is `criterion_name`.
fn with(text: &str, criterion_name: &str) -> Document {
    let mut document = small_document();
    document.terms = vec![term(50, "record", &["entry", "change set"])];
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements.push(requirement(
        key(60),
        text,
        vec![criterion(key(61), criterion_name)],
    ));
    document
}

/// The alias each `LQ-010` finding names, with the keys of its subjects.
fn aliases(document: &Document) -> Vec<(String, Vec<loftspec::identity::Key>)> {
    let all = findings(document);
    of(&all, RuleId::Lq010)
        .into_iter()
        .map(|finding| (finding.term.clone().unwrap(), keys(finding)))
        .collect()
}

#[test]
fn an_alias_in_prose_yields_one_finding_per_node_and_alias_naming_the_term() {
    let document = with(
        "The writer shall write an entry, and then another entry.",
        "The entry is written.",
    );
    let all = findings(&document);
    let found = of(&all, RuleId::Lq010);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(keys(found[0]), [key(60)]);
    assert_eq!(keys(found[1]), [key(61)]);
    assert!(found.iter().all(|f| f.term.as_deref() == Some("entry")));
    assert!(found.iter().all(|f| f.message.contains("`record`")));
    assert_eq!(severity(RuleId::Lq010), Severity::Warning);
}

#[test]
fn the_name_and_body_of_every_node_and_each_statement_are_read() {
    let mut document = with("The writer shall write keys.", "Keys are written.");
    document.products[0].name = "Entry Product".to_owned();
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some("Each entry has a key.\n".to_owned());
    document.components[0].component.name = "Entry store".to_owned();
    let found = aliases(&document);
    assert_eq!(found.len(), 3, "{found:?}");
    let body = findings(&document)
        .into_iter()
        .filter(|f| f.rule == RuleId::Lq010)
        .find(|f| f.body_line.is_some());
    assert_eq!(body.unwrap().body_line, Some(1));
}

#[test]
fn an_alias_matches_whole_words_in_any_case_with_or_without_a_plural() {
    let found = |text: &str| aliases(&with(text, "It holds.")).len();
    assert_eq!(found("The writer shall write Entries."), 1);
    assert_eq!(found("The writer shall write ENTRY lines."), 1);
    assert_eq!(found("The writer shall write an entryway."), 0);
    assert_eq!(found("The writer shall write a reentry."), 0);
}

#[test]
fn an_alias_of_two_words_matches_them_across_any_run_of_spaces() {
    let mut document = with("The writer shall write a change   set.", "It holds.");
    added(&mut document).details.body = Some("Two change\nsets.\n".to_owned());
    let found = aliases(&document);
    assert_eq!(found, [("change set".to_owned(), vec![key(60)])]);
}

#[test]
fn an_alias_in_a_code_span_or_code_block_is_not_reported() {
    let mut document = with("The writer shall write an `entry` field.", "It holds.");
    added(&mut document).details.body = Some("```\nentry\n```\n\nA `entry` span.\n".to_owned());
    assert_eq!(aliases(&document), []);
}

#[test]
fn a_terms_own_name_in_prose_is_not_reported() {
    let document = with("The writer shall write a record.", "Records are written.");
    assert_eq!(aliases(&document), []);
}

#[test]
fn an_empty_term_list_yields_no_alias_finding() {
    let mut document = with("The writer shall write an entry.", "It holds.");
    document.terms.clear();
    assert_eq!(aliases(&document), []);
}

/// The `SH-011` findings of a document whose term list is `terms`.
fn shared(terms: Vec<TermEntry>) -> Vec<(Option<String>, Vec<loftspec::identity::Key>)> {
    let mut document = small_document();
    document.terms = terms;
    let all = findings(&document);
    of(&all, RuleId::Sh011)
        .into_iter()
        .map(|finding| (finding.term.clone(), keys(finding)))
        .collect()
}

#[test]
fn a_name_or_alias_two_terms_carry_yields_one_finding_naming_both() {
    let found = shared(vec![
        term(50, "record", &["entry"]),
        term(51, "row", &["entry"]),
        term(52, "table", &[]),
    ]);
    assert_eq!(found, [(Some("entry".to_owned()), vec![key(50), key(51)])]);
    let named = shared(vec![term(50, "record", &[]), term(51, "row", &["record"])]);
    assert_eq!(named, [(Some("record".to_owned()), vec![key(50), key(51)])]);
    assert_eq!(severity(RuleId::Sh011), Severity::Warning);
}

#[test]
fn an_alias_equal_to_its_own_terms_name_yields_one_finding_naming_the_term() {
    let found = shared(vec![term(50, "record", &["Records"])]);
    assert_eq!(found, [(Some("Records".to_owned()), vec![key(50)])]);
}

#[test]
fn names_and_aliases_are_compared_in_any_case_and_with_or_without_a_plural() {
    let found = shared(vec![
        term(50, "Audit Entry", &[]),
        term(51, "log line", &["audit entries"]),
    ]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        shared(vec![term(50, "record", &[]), term(51, "row", &[])]),
        []
    );
}

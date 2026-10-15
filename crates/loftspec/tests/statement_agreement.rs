//! The two readings of an EARS sentence: the language rule takes a sentence for a form by its
//! words in any letter case, the reader keeps slots only for the template's exact wording, and
//! the finding for a statement stored as text names the form a sentence follows.

use crate::common::lint::{configured, findings, of};
use crate::common::{criterion, key, requirement, small_document};
use loftspec::document::Document;
use loftspec::lint::{Finding, RuleId};
use loftspec::statement::Statement;

const LQ_007_ON: &str = "rules:\n  - rule: LQ-007\n    enabled: true\n";

/// The small document with one more Requirement, its statement stored as text.
fn with_text_statement(text: &str) -> Document {
    let mut document = small_document();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements.push(requirement(
        key(60),
        text,
        vec![criterion(key(61), "It holds.")],
    ));
    document
}

/// The term of the one `V2-002` finding on the added Requirement.
fn v2_002_term(sentence: &str) -> Option<String> {
    let findings = findings(&with_text_statement(sentence));
    let found = of(&findings, RuleId::V2002);
    assert_eq!(found.len(), 1, "one V2-002 for `{sentence}`");
    found[0].term.clone()
}

/// Sentences that differ from a template only in the letter case of an anchor word, with the
/// form each follows.
const CASE_VARIANTS: [(&str, &str); 4] = [
    ("The writer SHALL write a key on every node.", "ubiquitous"),
    ("The writer Shall write a key on every node.", "ubiquitous"),
    (
        "WHEN a node has no key, the formatter shall mint one.",
        "event_driven",
    ),
    (
        "If a key is missing, THEN the reader shall refuse the file.",
        "unwanted",
    ),
];

#[test]
fn the_language_rule_takes_a_sentence_for_a_form_in_any_letter_case() {
    for (sentence, _) in CASE_VARIANTS {
        let findings = configured(&with_text_statement(sentence), LQ_007_ON);
        assert_eq!(
            of(&findings, RuleId::Lq007),
            Vec::<&Finding>::new(),
            "`{sentence}`"
        );
    }
}

#[test]
fn the_reader_keeps_slots_only_for_the_templates_exact_wording() {
    for (sentence, _) in CASE_VARIANTS {
        assert_eq!(
            Statement::parse(sentence),
            Statement::Text(sentence.to_owned()),
            "`{sentence}`"
        );
    }
}

#[test]
fn a_text_statement_that_follows_a_form_but_not_its_wording_is_v2_002_naming_the_form() {
    for (sentence, form) in CASE_VARIANTS {
        assert_eq!(v2_002_term(sentence).as_deref(), Some(form), "`{sentence}`");
    }
}

#[test]
fn a_text_statement_that_follows_no_form_is_v2_002_without_a_term() {
    assert_eq!(v2_002_term("Keys should mostly work."), None);
}

//! The rules of the writing craft `V2-005`..`V2-008` (F-21): a compound criterion, an oversized
//! Requirement, normative text in a body, and an article before a proper name.

use crate::common::lint::{added, configured, findings, keys, of, severity};
use crate::common::{criterion, key, requirement, section, small_document};
use loftspec::document::Document;
use loftspec::lint::{Finding, LintConfig, RuleId, Severity, count_body_rule};
use loftspec::statement::{EarsForm, Statement};
use loftspec::tree::Criterion;

/// The small document with one more Requirement under F-1, keyed 60, holding `criteria`.
fn with_criteria(criteria: Vec<Criterion>) -> Document {
    let mut document = small_document();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements.push(requirement(
        key(60),
        "The writer shall write keys.",
        criteria,
    ));
    document
}

/// The small document with one more Requirement whose one criterion is named `name`, keyed 61.
fn with_criterion(name: &str) -> Document {
    with_criteria(vec![criterion(key(61), name)])
}

/// `count` criteria keyed from 100.
fn criteria(count: u32) -> Vec<Criterion> {
    (0..count)
        .map(|n| criterion(key(100 + n), &format!("Outcome {n} holds.")))
        .collect()
}

/// `count` words of prose.
fn words(count: usize) -> String {
    vec!["word"; count].join(" ")
}

/// Asserts that a criterion named `name` is one `V2-005` finding, which names the criterion.
fn assert_compound(name: &str) {
    let document = with_criterion(name);
    let all = findings(&document);
    let compound = of(&all, RuleId::V2005);
    assert_eq!(compound.len(), 1, "{name}");
    assert_eq!(keys(compound[0]), [key(61)], "{name}");
}

#[test]
fn a_criterion_name_with_a_semicolon_is_v2_005() {
    assert_compound("The key is written; the label is kept.");
    assert_eq!(severity(RuleId::V2005), Severity::Warning);
}

#[test]
fn a_criterion_name_of_two_sentences_is_v2_005() {
    assert_compound("The key is written. The label is kept.");
}

#[test]
fn a_question_mark_or_an_exclamation_mark_before_a_space_and_a_capital_ends_a_sentence() {
    assert_compound("Is the key kept? The label is.");
    assert_compound("The key is kept! The label is.");
}

#[test]
fn a_sentence_mark_without_a_space_and_a_capital_after_it_ends_no_sentence() {
    for name in [
        "Is the key kept? the label is.",
        "The key is kept! the label is.",
        "The key is kept.The label is.",
        "Is the key kept?The label is.",
        "The key is kept!The label is.",
    ] {
        let document = with_criterion(name);
        assert_eq!(
            of(&findings(&document), RuleId::V2005),
            Vec::<&Finding>::new(),
            "{name}"
        );
    }
}

#[test]
fn a_criterion_name_of_one_sentence_is_not_v2_005() {
    let document = with_criterion("The key is written and the label is kept.");
    assert_eq!(
        of(&findings(&document), RuleId::V2005),
        Vec::<&Finding>::new()
    );
}

#[test]
fn a_full_stop_before_a_lowercase_word_or_in_a_code_span_ends_no_sentence() {
    for name in [
        "The file holds, e.g. a key, once.",
        "The error names `a.b; C. D` once.",
        "Version 2. that is all.",
    ] {
        let document = with_criterion(name);
        assert_eq!(
            of(&findings(&document), RuleId::V2005),
            Vec::<&Finding>::new(),
            "{name}"
        );
    }
}

#[test]
fn a_requirement_with_more_criteria_than_the_limit_is_v2_006() {
    let document = with_criteria(criteria(21));
    let all = findings(&document);
    let oversized = of(&all, RuleId::V2006);
    assert_eq!(oversized.len(), 1);
    assert_eq!(keys(oversized[0]), [key(60)]);
    assert!(
        oversized[0].message.contains("21"),
        "{}",
        oversized[0].message
    );
    assert_eq!(severity(RuleId::V2006), Severity::Info);
}

#[test]
fn a_requirement_at_both_limits_is_not_v2_006() {
    let mut document = with_criteria(criteria(20));
    added(&mut document).details.body = Some(words(150));
    assert_eq!(
        of(&findings(&document), RuleId::V2006),
        Vec::<&Finding>::new()
    );
}

#[test]
fn a_requirement_body_longer_than_the_limit_is_v2_006() {
    let mut document = with_criteria(criteria(1));
    added(&mut document).details.body = Some(words(151));
    let all = findings(&document);
    let oversized = of(&all, RuleId::V2006);
    assert_eq!(oversized.len(), 1);
    assert_eq!(keys(oversized[0]), [key(60)]);
    assert!(
        oversized[0].message.contains("151"),
        "{}",
        oversized[0].message
    );
}

#[test]
fn words_in_code_are_not_counted_toward_the_body_limit() {
    let mut document = with_criteria(criteria(1));
    let code = format!(
        "{}\n\n`{}`\n\n```\n{}\n```\n",
        words(100),
        words(60),
        words(60)
    );
    added(&mut document).details.body = Some(code);
    assert_eq!(
        of(&findings(&document), RuleId::V2006),
        Vec::<&Finding>::new()
    );
}

#[test]
fn the_limits_of_v2_006_are_set_in_the_lint_configuration() {
    let mut document = with_criteria(criteria(3));
    added(&mut document).details.body = Some(words(11));
    let yaml = "rules:\n  - rule: V2-006\n    maxCriteria: 2\n    maxBodyWords: 10\n";
    let all = configured(&document, yaml);
    let terms: Vec<_> = of(&all, RuleId::V2006)
        .iter()
        .map(|finding| finding.term.clone().unwrap())
        .collect();
    assert_eq!(terms, ["body", "criteria"]);
}

#[test]
fn a_normative_word_in_a_body_is_v2_007_at_each_occurrence_and_its_line() {
    let mut document = with_criterion("The key is written.");
    added(&mut document).details.body =
        Some("The writer must sort keys.\n\nIt Shall not stop.\n".to_owned());
    let all = findings(&document);
    let normative = of(&all, RuleId::V2007);
    let found: Vec<_> = normative
        .iter()
        .map(|f| (keys(f), f.term.clone().unwrap(), f.body_line))
        .collect();
    assert_eq!(
        found,
        [
            (vec![key(60)], "must".to_owned(), Some(1)),
            (vec![key(60)], "shall".to_owned(), Some(3)),
        ]
    );
    assert_eq!(severity(RuleId::V2007), Severity::Warning);
}

#[test]
fn a_normative_word_in_a_sections_body_is_v2_007_naming_the_section() {
    let mut document = with_criterion("The key is written.");
    added(&mut document).criteria[0].details.sections =
        vec![section(62, "Notes", "It must stop.\nIt must, it must.\n")];
    let all = findings(&document);
    let found: Vec<_> = of(&all, RuleId::V2007)
        .iter()
        .map(|f| (keys(f), f.term.clone().unwrap(), f.body_line))
        .collect();
    let must = |line| (vec![key(62)], "must".to_owned(), Some(line));
    assert_eq!(found, [must(1), must(2), must(2)]);
}

#[test]
fn a_body_without_a_normative_word_is_not_v2_007() {
    let mut document = with_criterion("The key is written.");
    added(&mut document).details.body = Some("The writer sorts keys.".to_owned());
    assert_eq!(
        of(&findings(&document), RuleId::V2007),
        Vec::<&Finding>::new()
    );
}

#[test]
fn a_normative_word_in_code_or_inside_another_word_is_not_v2_007() {
    let mut document = with_criterion("The key is written.");
    added(&mut document).details.body =
        Some("The `shall` slot is shallow.\n\n```\nmust\n```\n".to_owned());
    assert_eq!(
        of(&findings(&document), RuleId::V2007),
        Vec::<&Finding>::new()
    );
}

#[test]
fn the_lexicon_of_v2_007_is_edited_in_the_lint_configuration() {
    let mut document = with_criterion("The key is written.");
    added(&mut document).details.body = Some("It must sort and should stop.".to_owned());
    let yaml = "rules:\n  - rule: V2-007\n    lexicon:\n      extend:\n        - \"should\"\n      remove:\n        - \"must\"\n";
    let all = configured(&document, yaml);
    let terms: Vec<_> = of(&all, RuleId::V2007)
        .iter()
        .map(|finding| finding.term.clone().unwrap())
        .collect();
    assert_eq!(terms, ["should"]);
}

#[test]
fn counting_one_body_rule_counts_what_the_linter_reports_for_it() {
    let mut document = with_criterion("The key is written.");
    added(&mut document).details.body = Some("The writer must sort keys.".to_owned());
    added(&mut document).criteria[0].details.sections =
        vec![section(62, "Notes", "It must stop.\nIt must, it must.\n")];
    let config = LintConfig::default();
    let reported = of(&findings(&document), RuleId::V2007).len();
    assert_eq!(reported, 4);
    assert_eq!(count_body_rule(&document, &config, RuleId::V2007), reported);
}

#[test]
fn counting_a_rule_that_does_not_read_bodies_alone_counts_nothing() {
    let mut document = with_criterion("The key is written.");
    added(&mut document).details.body = Some("The writer sorts many keys.".to_owned());
    let config = LintConfig::default();
    assert_eq!(of(&findings(&document), RuleId::Lq002).len(), 1);
    assert_eq!(count_body_rule(&document, &config, RuleId::Lq002), 0);
}

/// The small document with one more Requirement whose statement's system is `system`.
fn with_system(system: &str) -> Document {
    let mut document = with_criterion("The key is written.");
    added(&mut document).statement = Statement::Form(EarsForm::Ubiquitous {
        system: system.to_owned(),
        shall: "write keys.".to_owned(),
    });
    document
}

const PROPER_NAMES: &str =
    "rules:\n  - rule: V2-008\n    properNames:\n      - \"Loft\"\n      - \"Software Plant\"\n";

#[test]
fn a_proper_name_after_an_article_is_v2_008() {
    for (system, name) in [
        ("The Loft", "Loft"),
        ("the Software Plant", "Software Plant"),
    ] {
        let all = configured(&with_system(system), PROPER_NAMES);
        let article = of(&all, RuleId::V2008);
        assert_eq!(article.len(), 1, "{system}");
        assert_eq!(keys(article[0]), [key(60)]);
        assert_eq!(article[0].term.as_deref(), Some(name));
    }
    assert_eq!(severity(RuleId::V2008), Severity::Info);
}

#[test]
fn a_bare_proper_name_or_another_name_is_not_v2_008() {
    for system in ["Loft", "The loft", "The Loft binary", "The writer"] {
        let all = configured(&with_system(system), PROPER_NAMES);
        assert_eq!(of(&all, RuleId::V2008), Vec::<&Finding>::new(), "{system}");
    }
}

#[test]
fn without_proper_names_v2_008_is_silent() {
    assert_eq!(
        of(&findings(&with_system("The Loft")), RuleId::V2008),
        Vec::<&Finding>::new()
    );
}

#[test]
fn proper_names_must_be_a_list_of_names() {
    for yaml in [
        "rules:\n  - rule: V2-008\n    properNames: \"Loft\"\n",
        "rules:\n  - rule: V2-008\n    properNames:\n      - \"\"\n",
        "rules:\n  - rule: V2-005\n    properNames:\n      - \"Loft\"\n",
    ] {
        assert!(LintConfig::read(Some(yaml.as_bytes())).is_err(), "{yaml}");
    }
}

#[test]
fn proper_names_written_with_no_value_are_empty() {
    let yaml = "rules:\n  - rule: V2-008\n    properNames:\n";
    let config = LintConfig::read(Some(yaml.as_bytes())).unwrap();
    assert_eq!(config.proper_names(), Vec::<String>::new());
    assert_eq!(config, LintConfig::default());
}

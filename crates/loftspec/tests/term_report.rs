//! The term report: the phrases it reads, the groups `TD-001`, `TD-002` and `TD-003` form, the
//! groups the term list decides, the order of the report, and the subjects of each Product.

use crate::common::{criterion, key, requirement, small_document};
use loftspec::document::{Document, TermEntry};
use loftspec::ids::TermId;
use loftspec::lint::report::{Group, Mark, Subject};
use loftspec::lint::{FUNCTION_WORDS, LintConfig, RuleId, Severity, term_report};
use loftspec::statement::{EarsForm, Statement};

/// The small document with one more Requirement under F-1, whose criteria are `texts`.
fn with_criteria(texts: &[&str]) -> Document {
    let mut document = small_document();
    let criteria = texts
        .iter()
        .zip(61..)
        .map(|(text, n)| criterion(key(n), text))
        .collect();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements.push(requirement(
        key(60),
        "The writer shall keep notes.",
        criteria,
    ));
    document
}

fn report_of(document: &Document, config: &LintConfig) -> Vec<Group> {
    term_report(document, config).groups
}

fn groups(document: &Document) -> Vec<Group> {
    report_of(document, &LintConfig::default())
}

fn configured(document: &Document, yaml: &str) -> Vec<Group> {
    report_of(document, &LintConfig::read(Some(yaml.as_bytes())).unwrap())
}

/// Each group of `rule` as its phrases' texts and counts.
fn of(groups: &[Group], rule: RuleId) -> Vec<Vec<(String, usize)>> {
    groups
        .iter()
        .filter(|group| group.rule == rule)
        .map(|group| {
            group
                .phrases
                .iter()
                .map(|phrase| (phrase.text.clone(), phrase.count))
                .collect()
        })
        .collect()
}

fn pairs(list: &[(&str, usize)]) -> Vec<(String, usize)> {
    list.iter()
        .map(|(text, n)| ((*text).to_owned(), *n))
        .collect()
}

fn term(n: u32, name: &str, aliases: &[&str]) -> TermEntry {
    TermEntry {
        id: TermId::new(key(n)),
        name: name.to_owned(),
        definition: format!("What {name} means."),
        aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
    }
}

const AUDIT: [&str; 3] = [
    "The audit entry is kept.",
    "The audit row is kept.",
    "The audit entry is shown.",
];

#[test]
fn names_bodies_and_statements_of_every_node_are_read() {
    let mut document = with_criteria(&["The audit entry is kept."]);
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.details.body = Some("Each audit row is kept.\n".to_owned());
    feature.requirements[1].statement = Statement::Text("The audit record shall be kept.".into());
    let found = of(&groups(&document), RuleId::Td001);
    assert_eq!(
        found,
        [pairs(&[
            ("audit entry", 1),
            ("audit record", 1),
            ("audit row", 1)
        ])]
    );
}

#[test]
fn text_in_a_code_span_or_a_fenced_code_block_is_not_read() {
    let mut document = with_criteria(&["The `audit row` is kept.", "The audit entry is kept."]);
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.details.body = Some("```\naudit row\naudit row\n```\n\nAn audit entry.\n".to_owned());
    assert_eq!(of(&groups(&document), RuleId::Td001), Vec::<Vec<_>>::new());
}

#[test]
fn a_phrase_is_one_to_three_words_without_punctuation_between_them() {
    let document = with_criteria(&[
        "The audit, entry is kept.",
        "The audit, row is kept.",
        "The audit; entry is kept.",
    ]);
    assert_eq!(of(&groups(&document), RuleId::Td001), Vec::<Vec<_>>::new());
    let long = with_criteria(&[
        "Every big tenant admin role list is kept.",
        "Every big tenant administrator role list is kept.",
    ]);
    for group in groups(&long) {
        for phrase in group.phrases {
            assert!(phrase.text.split(' ').count() <= 3, "{}", phrase.text);
        }
    }
}

#[test]
fn a_possessive_ends_a_run() {
    let document = with_criteria(&[
        "The node's entry is kept.",
        "The node's row is kept.",
        "The node's entry is shown.",
    ]);
    assert_eq!(of(&groups(&document), RuleId::Td001), Vec::<Vec<_>>::new());
}

#[test]
fn a_phrase_neither_begins_nor_ends_with_a_function_word() {
    let document = with_criteria(&[
        "The entry is kept.",
        "The row is kept.",
        "The entry is shown.",
    ]);
    assert_eq!(groups(&document), []);
}

#[test]
fn the_function_words_are_the_published_list() {
    let published = include_str!("../../../testdata/golden/function-words.txt");
    let listed: Vec<&str> = published.lines().collect();
    assert_eq!(listed, FUNCTION_WORDS);
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    assert_eq!(listed, sorted);
}

#[test]
fn phrases_that_differ_in_case_or_a_final_plural_are_one_phrase() {
    let document = with_criteria(&[
        "The Audit Entries are kept.",
        "The audit row is kept.",
        "The audit entry is shown.",
    ]);
    let found = of(&groups(&document), RuleId::Td001);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0][0].1, 2, "{found:?}");
    assert_eq!(found[0][1], ("audit row".to_owned(), 1));
}

#[test]
fn one_qualifier_before_head_nouns_of_one_noun_group_forms_a_td_001_group() {
    let all = groups(&with_criteria(&AUDIT));
    assert_eq!(
        of(&all, RuleId::Td001),
        [pairs(&[("audit entry", 2), ("audit row", 1)])]
    );
    assert!(all[0].message.contains("`record`"), "{}", all[0].message);
    assert_eq!(all[0].severity, Severity::Hint);
}

#[test]
fn a_td_001_group_used_fewer_than_three_times_is_not_reported() {
    let document = with_criteria(&AUDIT[..2]);
    assert_eq!(of(&groups(&document), RuleId::Td001), Vec::<Vec<_>>::new());
}

#[test]
fn a_qualifier_that_is_a_noun_group_word_forms_no_td_001_group() {
    let document = with_criteria(&[
        "The entry record is kept.",
        "The entry row is kept.",
        "The entry record is shown.",
    ]);
    assert_eq!(of(&groups(&document), RuleId::Td001), Vec::<Vec<_>>::new());
}

#[test]
fn the_noun_groups_are_the_published_ones_unless_the_configuration_edits_them() {
    let config = LintConfig::default();
    let names: Vec<&str> = config.noun_groups().keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "document",
            "failure",
            "identifier",
            "record",
            "request",
            "system",
            "view"
        ]
    );
    let removed = "rules:\n  - rule: TD-001\n    nounGroups:\n      remove:\n        - name: \"record\"\n          words:\n            - \"row\"\n";
    let document = with_criteria(&AUDIT);
    assert_eq!(
        of(&configured(&document, removed), RuleId::Td001),
        Vec::<Vec<_>>::new()
    );
    let extended = "rules:\n  - rule: TD-001\n    nounGroups:\n      extend:\n        - name: \"place\"\n          words:\n            - \"shelf\"\n            - \"rack\"\n";
    let racks = with_criteria(&["The book shelf.", "The book rack.", "The book shelf again."]);
    assert_eq!(of(&configured(&racks, extended), RuleId::Td001).len(), 1);
}

#[test]
fn one_name_written_as_one_word_and_as_two_forms_a_td_002_group() {
    let all = groups(&with_criteria(&[
        "The change set is kept.",
        "The change set is shown.",
        "The changeset is sent.",
    ]));
    assert_eq!(
        of(&all, RuleId::Td002),
        [pairs(&[("change set", 2), ("changeset", 1)])]
    );
    assert_eq!(all[0].severity, Severity::Hint);
    assert!(!all[0].message.contains("identifier"), "{}", all[0].message);
}

#[test]
fn a_td_002_group_used_fewer_than_three_times_is_not_reported() {
    let document = with_criteria(&["The change set is kept.", "The changeset is sent."]);
    assert_eq!(of(&groups(&document), RuleId::Td002), Vec::<Vec<_>>::new());
}

#[test]
fn a_hyphen_in_place_of_a_space_forms_no_td_002_group() {
    let document = with_criteria(&[
        "The change-set is kept.",
        "The change set is shown.",
        "The change-set is sent.",
    ]);
    assert_eq!(of(&groups(&document), RuleId::Td002), Vec::<Vec<_>>::new());
}

#[test]
fn a_camel_case_single_word_is_reported_as_an_identifier_written_as_prose() {
    let all = groups(&with_criteria(&[
        "The ChangeSet is kept.",
        "The change set is shown.",
        "The ChangeSet is sent.",
    ]));
    let group = all
        .iter()
        .find(|group| group.rule == RuleId::Td002)
        .unwrap();
    assert!(
        group.message.contains("`ChangeSet` is an identifier"),
        "{}",
        group.message
    );
}

#[test]
fn a_phrase_with_an_abbreviation_and_in_full_forms_a_td_003_group() {
    let all = groups(&with_criteria(&[
        "The tenant admin is told.",
        "The tenant administrator is told.",
    ]));
    assert_eq!(
        of(&all, RuleId::Td003),
        [pairs(&[("tenant admin", 1), ("tenant administrator", 1)])]
    );
    let group = all
        .iter()
        .find(|group| group.rule == RuleId::Td003)
        .unwrap();
    assert!(
        group
            .message
            .contains("`admin` abbreviates `administrator`")
    );
    assert_eq!(group.severity, Severity::Hint);
}

#[test]
fn a_single_word_and_its_long_form_form_no_td_003_group() {
    let document = with_criteria(&["The admin is told.", "The administrator is told."]);
    assert_eq!(of(&groups(&document), RuleId::Td003), Vec::<Vec<_>>::new());
}

#[test]
fn the_pairs_are_the_published_ones_unless_the_configuration_edits_them() {
    let config = LintConfig::default();
    assert!(config.pairs()["spec"].contains("specification"));
    assert!(config.pairs()["admin"].contains("administration"));
    let document = with_criteria(&[
        "The tenant admin is told.",
        "The tenant administrator is told.",
    ]);
    let removed =
        "rules:\n  - rule: TD-003\n    pairs:\n      remove:\n        - name: \"admin\"\n";
    assert_eq!(
        of(&configured(&document, removed), RuleId::Td003),
        Vec::<Vec<_>>::new()
    );
}

#[test]
fn a_group_the_term_list_decides_is_not_reported_and_one_it_does_not_is() {
    let mut document = with_criteria(&AUDIT);
    document.terms = vec![term(50, "audit entry", &["audit row"])];
    assert_eq!(groups(&document), []);
    document.terms = vec![term(50, "audit entry", &[])];
    assert_eq!(of(&groups(&document), RuleId::Td001).len(), 1);
    document.terms = vec![term(50, "Audit Entries", &["AUDIT ROWS"])];
    assert_eq!(groups(&document), []);
}

#[test]
fn each_phrase_is_given_with_its_count_and_up_to_three_nodes_in_document_order() {
    let document = with_criteria(&[
        "The audit row is kept.",
        "The audit entry is kept.",
        "The audit entry is shown.",
        "The audit entry is sent.",
        "The audit entry is read.",
    ]);
    let all = groups(&document);
    let entry = &all[0].phrases[0];
    assert_eq!((entry.text.as_str(), entry.count), ("audit entry", 4));
    assert_eq!(entry.nodes, [key(62), key(63), key(64)]);
}

#[test]
fn groups_are_ordered_by_their_second_phrase_and_phrases_by_count_then_text() {
    let document = with_criteria(&[
        "The audit row is kept.",
        "The audit entry is kept.",
        "The audit entry is shown.",
        "The change set is kept.",
        "The changeset is sent.",
        "The changeset is read.",
        "The change set is told.",
        "The audit item is kept.",
    ]);
    let all = groups(&document);
    let order: Vec<RuleId> = all.iter().map(|group| group.rule).collect();
    assert_eq!(order, [RuleId::Td002, RuleId::Td001]);
    assert_eq!(
        of(&all, RuleId::Td001),
        [pairs(&[
            ("audit entry", 2),
            ("audit item", 1),
            ("audit row", 1)
        ])]
    );
}

/// The small document's Product with formed statements whose systems are `systems`.
fn with_systems(systems: &[&str]) -> Document {
    let mut document = small_document();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    for (system, n) in systems.iter().zip(70..) {
        let mut added = requirement(key(n), "", vec![criterion(key(n + 100), "It holds.")]);
        added.statement = Statement::Form(EarsForm::Ubiquitous {
            system: (*system).to_owned(),
            shall: "keep notes.".to_owned(),
        });
        feature.requirements.push(added);
    }
    document
}

fn subjects(document: &Document) -> Vec<Subject> {
    let report = term_report(document, &LintConfig::default());
    report.products[0].subjects.clone()
}

#[test]
fn the_subjects_of_a_product_are_the_systems_of_its_statements_counted() {
    let document = with_systems(&["The writer", "the writer", "Writer", "The note store"]);
    let found: Vec<(String, usize)> = subjects(&document)
        .into_iter()
        .map(|subject| (subject.text, subject.count))
        .collect();
    // The small document's own Requirement names the writer too.
    assert_eq!(found, pairs(&[("writer", 4), ("note store", 1)]));
    let report = term_report(&document, &LintConfig::default());
    assert_eq!(report.products[0].product, Some(key(1)));
}

#[test]
fn a_subject_is_marked_as_a_term_an_alias_or_a_node_name() {
    let mut document = with_systems(&[
        "The writer",
        "The scribe",
        "The Example Product",
        "a stranger",
    ]);
    document.terms = vec![term(50, "writer", &["scribe"])];
    let marks: Vec<(String, Option<Mark>)> = subjects(&document)
        .into_iter()
        .map(|subject| (subject.text, subject.mark))
        .collect();
    assert_eq!(
        marks,
        [
            ("writer".to_owned(), Some(Mark::Term)),
            ("Example Product".to_owned(), Some(Mark::Node)),
            ("a stranger".to_owned(), None),
            ("scribe".to_owned(), Some(Mark::Alias("writer".to_owned()))),
        ]
    );
}

#[test]
fn two_runs_give_the_same_report() {
    let document = with_criteria(&AUDIT);
    let config = LintConfig::default();
    assert_eq!(
        term_report(&document, &config),
        term_report(&document, &config)
    );
}

#[test]
fn the_rules_of_the_term_report_are_configured_like_the_lint_rules() {
    let document = with_criteria(&AUDIT);
    let off = "rules:\n  - rule: TD-001\n    enabled: false\n";
    assert_eq!(configured(&document, off), []);
    let info = "rules:\n  - rule: TD-001\n    severity: info\n";
    assert_eq!(configured(&document, info)[0].severity, Severity::Info);
}

#[test]
fn a_check_reports_no_finding_of_the_term_report() {
    let document = with_criteria(&AUDIT);
    let findings = crate::common::lint::findings(&document);
    assert!(
        findings
            .iter()
            .all(|finding| !RuleId::TERM_REPORT.contains(&finding.rule)),
        "{findings:?}"
    );
}

#[test]
fn a_phrase_of_three_words_holding_a_pair_of_two_forms_no_td_003_group_of_its_own() {
    let all = groups(&with_criteria(&[
        "The tenant admin role is told.",
        "The tenant administrator role is told.",
    ]));
    let found = of(&all, RuleId::Td003);
    assert!(
        found.iter().all(|group| group[0].0.split(' ').count() == 2),
        "{found:?}"
    );
    assert!(found.contains(&pairs(&[("tenant admin", 1), ("tenant administrator", 1)])));
}

#[test]
fn groups_are_replaced_or_extended_and_reduced_each_entry_a_name_and_its_words() {
    let replaced = "rules:\n  - rule: TD-001\n    nounGroups:\n      replace:\n        - name: \"place\"\n          words:\n            - \"shelf\"\n            - \"rack\"\n";
    let config = LintConfig::read(Some(replaced.as_bytes())).unwrap();
    let names: Vec<&str> = config.noun_groups().keys().map(String::as_str).collect();
    assert_eq!(names, ["place"]);
    let both = "rules:\n  - rule: TD-001\n    nounGroups:\n      replace:\n        - name: \"place\"\n          words:\n            - \"shelf\"\n      remove:\n        - name: \"view\"\n";
    assert!(LintConfig::read(Some(both.as_bytes())).is_err());
    let nameless = "rules:\n  - rule: TD-003\n    pairs:\n      extend:\n        - words:\n            - \"x\"\n";
    assert!(LintConfig::read(Some(nameless.as_bytes())).is_err());
    let extended = "rules:\n  - rule: TD-003\n    pairs:\n      extend:\n        - name: \"admin\"\n          words:\n            - \"admins\"\n";
    let config = LintConfig::read(Some(extended.as_bytes())).unwrap();
    assert!(config.pairs()["admin"].contains("administrator"));
    assert!(config.pairs()["admin"].contains("admins"));
    let two_words = "rules:\n  - rule: TD-003\n    pairs:\n      extend:\n        - name: \"ui\"\n          words:\n            - \"user interface\"\n";
    assert!(LintConfig::read(Some(two_words.as_bytes())).is_err());
}

#[test]
fn the_term_reporter_changes_nothing_in_the_document() {
    let document = with_criteria(&AUDIT);
    let before = document.clone();
    let _ = term_report(&document, &LintConfig::default());
    assert_eq!(document, before);
}

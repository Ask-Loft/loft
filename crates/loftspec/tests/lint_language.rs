//! The language rules `LQ-001`..`LQ-009` and the texts they read (D-11 F-20).

mod common;

use common::{component, criterion, key, requirement, root, small_document};
use loftspec::codec::Sites;
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::lint::{Finding, LintConfig, RuleId, Severity, lint};
use loftspec::statement::Statement;
use loftspec::text::tokenize;
use loftspec::tree::Requirement;

fn findings(document: &Document) -> Vec<Finding> {
    lint(document, &Sites::default(), &LintConfig::default())
}

fn configured(document: &Document, yaml: &str) -> Vec<Finding> {
    let config = LintConfig::read(Some(yaml.as_bytes())).unwrap();
    lint(document, &Sites::default(), &config)
}

fn of(findings: &[Finding], rule: RuleId) -> Vec<&Finding> {
    findings
        .iter()
        .filter(|finding| finding.rule == rule)
        .collect()
}

fn keys(finding: &Finding) -> Vec<Key> {
    finding.subjects.iter().map(|subject| subject.key).collect()
}

fn severity(rule: RuleId) -> Severity {
    LintConfig::default().severity(rule)
}

/// The small document with one more Requirement under F-1, its statement stored as text.
fn with_statement(text: &str) -> Document {
    let mut document = small_document();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements.push(requirement(
        key(60),
        text,
        vec![criterion(key(61), "It holds.")],
    ));
    document
}

fn added(document: &mut Document) -> &mut Requirement {
    &mut document.products[0].feature_groups[0].features[0].requirements[1]
}

#[test]
fn ac_370_ac_373_ac_376_a_weak_term_in_a_statement_names_the_requirement_and_the_term() {
    let document = with_statement("The writer shall be fast and robust.");
    let all = findings(&document);
    let weak = of(&all, RuleId::Lq001);
    let terms: Vec<_> = weak.iter().map(|f| f.term.as_deref().unwrap()).collect();
    assert_eq!(terms, ["fast", "robust"]);
    assert!(
        weak.iter()
            .all(|f| keys(f) == [key(60)] && f.body_line.is_none())
    );
    assert_eq!(severity(RuleId::Lq001), Severity::Warning);
}

#[test]
fn ac_371_ac_872_a_finding_in_a_body_names_the_node_and_the_body_line() {
    let mut document = with_statement("The writer shall write keys.");
    added(&mut document).details.body =
        Some("First line.\n\nThe result is easy to read.\n".to_owned());
    let all = findings(&document);
    let weak = of(&all, RuleId::Lq001);
    assert_eq!(weak.len(), 1);
    assert_eq!(keys(weak[0]), [key(60)]);
    assert_eq!(weak[0].body_line, Some(3));
}

#[test]
fn ac_372_ac_651_a_criterion_is_read_as_a_statement_and_named_by_its_key() {
    let mut document = with_statement("The writer shall write keys.");
    added(&mut document).criteria[0].name = "It is user-friendly.".to_owned();
    let all = findings(&document);
    let weak = of(&all, RuleId::Lq001);
    assert_eq!(keys(weak[0]), [key(61)]);
    assert_eq!(weak[0].term.as_deref(), Some("user friendly"));
}

#[test]
fn ac_646_texts_split_into_the_published_tokens() {
    assert_eq!(
        tokenize("At most 65,536 keys; the `loft fmt` run—fast."),
        [
            "at", "most", "65,536", "keys", "the", "loft", "fmt", "run", "fast"
        ]
    );
}

#[test]
fn ac_375_a_term_removed_by_configuration_is_not_reported() {
    let document = with_statement("The writer shall be fast.");
    let yaml = "rules:\n  - rule: LQ-001\n    lexicon:\n      remove:\n        - fast\n";
    assert_eq!(
        of(&configured(&document, yaml), RuleId::Lq001),
        Vec::<&Finding>::new()
    );
}

#[test]
fn ac_424_ac_868_a_lexicon_can_be_extended() {
    let document = with_statement("The writer shall be snappy.");
    let yaml = "rules:\n  - rule: LQ-001\n    lexicon:\n      extend:\n        - snappy\n";
    assert_eq!(of(&configured(&document, yaml), RuleId::Lq001).len(), 1);
}

#[test]
fn ac_377_ac_380_a_vague_quantifier_is_lq_002_per_occurrence() {
    let document = with_statement("The writer shall keep some keys for several runs.");
    let all = findings(&document);
    let terms: Vec<_> = of(&all, RuleId::Lq002)
        .iter()
        .map(|f| f.term.clone().unwrap())
        .collect();
    assert_eq!(terms, ["several", "some"]);
    assert_eq!(severity(RuleId::Lq002), Severity::Warning);
}

#[test]
fn ac_378_ac_379_at_most_before_a_number_is_a_bound() {
    for text in [
        "The writer shall keep at most 8 keys.",
        "The writer shall keep at most 65,536 keys.",
        "The writer shall keep at most eight keys.",
    ] {
        assert!(
            of(&findings(&with_statement(text)), RuleId::Lq002).is_empty(),
            "{text}"
        );
    }
    let vague = with_statement("The writer shall keep at most keys.");
    assert_eq!(of(&findings(&vague), RuleId::Lq002).len(), 1);
}

#[test]
fn ac_381_ac_383_an_escape_clause_is_lq_003() {
    let document = with_statement("The writer shall write keys if possible.");
    let all = findings(&document);
    let clauses = of(&all, RuleId::Lq003);
    assert_eq!(clauses.len(), 1);
    assert_eq!(clauses[0].term.as_deref(), Some("if possible"));
    assert_eq!(severity(RuleId::Lq003), Severity::Warning);
}

#[test]
fn ac_382_a_statement_without_an_escape_clause_is_not_lq_003() {
    assert_eq!(
        of(&findings(&small_document()), RuleId::Lq003),
        Vec::<&Finding>::new()
    );
}

#[test]
fn ac_384_to_ac_386_absolute_language_is_lq_004_only_when_enabled() {
    let document = with_statement("The writer shall always write keys.");
    assert_eq!(
        of(&findings(&document), RuleId::Lq004),
        Vec::<&Finding>::new()
    );
    let enabled = configured(&document, "rules:\n  - rule: LQ-004\n    enabled: true\n");
    let absolute: Vec<_> = of(&enabled, RuleId::Lq004)
        .into_iter()
        .filter(|f| keys(f) == [key(60)])
        .collect();
    assert_eq!(absolute.len(), 1);
    assert_eq!(absolute[0].term.as_deref(), Some("always"));
    assert_eq!(absolute[0].severity, Severity::Info);
}

#[test]
fn ac_387_ac_390_a_statement_with_two_modal_verbs_is_lq_005() {
    let document = with_statement("The writer shall write keys and must sort them.");
    let all = findings(&document);
    assert_eq!(of(&all, RuleId::Lq005).len(), 1);
    assert_eq!(severity(RuleId::Lq005), Severity::Info);
}

#[test]
fn ac_388_ac_389_conjunctions_that_reach_the_threshold_are_lq_005() {
    let document = with_statement("The writer shall write keys and labels and names or slugs.");
    assert_eq!(of(&findings(&document), RuleId::Lq005).len(), 1);
    let two = with_statement("The writer shall write keys and labels or names.");
    assert_eq!(of(&findings(&two), RuleId::Lq005), Vec::<&Finding>::new());
    let lowered = configured(
        &two,
        "rules:\n  - rule: LQ-005\n    conjunctionThreshold: 2\n",
    );
    assert_eq!(of(&lowered, RuleId::Lq005).len(), 1);
}

#[test]
fn ac_391_to_ac_393_every_quality_attribute_carries_a_target_so_lq_006_is_silent() {
    assert_eq!(
        of(&findings(&small_document()), RuleId::Lq006),
        Vec::<&Finding>::new()
    );
    assert_eq!(severity(RuleId::Lq006), Severity::Warning);
}

#[test]
fn ac_394_ac_397_ac_398_a_text_statement_outside_the_forms_is_lq_007_when_enabled() {
    let document = with_statement("Keys everywhere, always.");
    assert_eq!(
        of(&findings(&document), RuleId::Lq007),
        Vec::<&Finding>::new()
    );
    let enabled = configured(&document, "rules:\n  - rule: LQ-007\n    enabled: true\n");
    let found = of(&enabled, RuleId::Lq007);
    assert_eq!(found.len(), 1);
    assert_eq!(keys(found[0]), [key(60)]);
    assert_eq!(found[0].severity, Severity::Info);
}

#[test]
fn ac_395_ac_396_slots_and_bare_proper_names_are_not_lq_007() {
    let mut document = with_statement("Loft shall refuse an unknown key.");
    let yaml = "rules:\n  - rule: LQ-007\n    enabled: true\n";
    assert_eq!(
        of(&configured(&document, yaml), RuleId::Lq007),
        Vec::<&Finding>::new()
    );
    added(&mut document).statement = Statement::parse("The writer shall write keys.");
    assert_eq!(
        of(&configured(&document, yaml), RuleId::Lq007),
        Vec::<&Finding>::new()
    );
}

#[test]
fn ac_399_ac_402_ac_644_two_near_duplicate_names_are_one_lq_008_naming_both() {
    let mut document = small_document();
    document
        .components
        .push(root(component(key(62), "Document model store index")));
    document
        .components
        .push(root(component(key(63), "Document model store index file")));
    let all = findings(&document);
    let near = of(&all, RuleId::Lq008);
    assert_eq!(near.len(), 1);
    assert_eq!(keys(near[0]), [key(62), key(63)]);
    assert_eq!(severity(RuleId::Lq008), Severity::Info);
}

#[test]
fn ac_644_similarity_counts_repeated_tokens() {
    let mut document = small_document();
    document
        .components
        .push(root(component(key(62), "key key key store")));
    document
        .components
        .push(root(component(key(63), "key store")));
    assert_eq!(
        of(&findings(&document), RuleId::Lq008),
        Vec::<&Finding>::new()
    );
}

#[test]
fn ac_400_ac_698_a_name_below_the_minimum_takes_part_only_when_equal() {
    let mut document = small_document();
    document.components.push(root(component(key(62), "Store")));
    document
        .components
        .push(root(component(key(63), "Store index")));
    let raised = configured(&document, "rules:\n  - rule: LQ-008\n    threshold: 0.5\n");
    assert_eq!(of(&raised, RuleId::Lq008), Vec::<&Finding>::new());
    let lowered = configured(
        &document,
        "rules:\n  - rule: LQ-008\n    threshold: 0.5\n    minTokens: 1\n",
    );
    assert_eq!(of(&lowered, RuleId::Lq008).len(), 1);
}

#[test]
fn ac_645_names_are_compared_only_within_one_kind() {
    let mut document = small_document();
    document.components.push(root(component(key(62), "Keys")));
    assert_eq!(
        of(&findings(&document), RuleId::Lq008),
        Vec::<&Finding>::new()
    );
}

#[test]
fn ac_403_ac_405_a_registry_term_the_node_does_not_list_is_lq_009() {
    let document = small_document();
    let all = findings(&document);
    let found = of(&all, RuleId::Lq009);
    assert_eq!(found.len(), 1);
    assert_eq!(keys(found[0]), [key(2)]);
    assert_eq!(found[0].term.as_deref(), Some("identity"));
    assert_eq!(severity(RuleId::Lq009), Severity::Hint);
}

#[test]
fn ac_404_a_node_that_lists_the_tag_or_sits_under_one_that_does_is_not_lq_009() {
    let mut document = small_document();
    let tag = loftspec::names::TagName::parse("identity").unwrap();
    document.products[0].details.tags.push(tag);
    assert_eq!(
        of(&findings(&document), RuleId::Lq009),
        Vec::<&Finding>::new()
    );
}

#[test]
fn ac_873_lq_009_does_not_read_a_constraint() {
    let mut document = small_document();
    let tag = loftspec::names::TagName::parse("identity").unwrap();
    document.products[0].details.tags.push(tag);
    document.components[0].constraints[0].name =
        "The document model shall keep identity rules.".to_owned();
    assert_eq!(
        of(&findings(&document), RuleId::Lq009),
        Vec::<&Finding>::new()
    );
}

#[test]
fn a_code_span_in_a_statement_is_not_read_as_prose() {
    let quoted =
        with_statement("The renderer shall write `<system> shall <shall>` for every form.");
    assert_eq!(
        of(&findings(&quoted), RuleId::Lq005),
        Vec::<&Finding>::new()
    );
    let weak = with_statement("The writer shall name the `fast` path.");
    assert_eq!(of(&findings(&weak), RuleId::Lq001), Vec::<&Finding>::new());
    let plain = with_statement("The writer shall be fast and name the `x` path.");
    let terms: Vec<_> = of(&findings(&plain), RuleId::Lq001)
        .iter()
        .map(|f| f.term.clone().unwrap())
        .collect();
    assert_eq!(terms, ["fast"]);
}

#[test]
fn a_code_span_closes_only_on_a_run_of_its_own_length() {
    let double = with_statement("The writer shall write ``a ` fast b`` here.");
    assert_eq!(
        of(&findings(&double), RuleId::Lq001),
        Vec::<&Finding>::new()
    );
    let unclosed = with_statement("The writer shall keep a ` mark and be fast.");
    assert_eq!(of(&findings(&unclosed), RuleId::Lq001).len(), 1);
}

#[test]
fn a_registry_term_inside_a_code_span_is_not_lq_009() {
    let mut document = small_document();
    document.products[0].feature_groups[0].name = "The `identity` group".to_owned();
    assert_eq!(
        of(&findings(&document), RuleId::Lq009),
        Vec::<&Finding>::new()
    );
}

/// The small document with one more Requirement under F-1 holding the given criteria.
fn with_criteria(texts: &[&str]) -> Document {
    let mut document = with_statement("The writer shall write the index.");
    added(&mut document).criteria = texts
        .iter()
        .enumerate()
        .map(|(n, text)| criterion(key(70 + u32::try_from(n).unwrap()), text))
        .collect();
    document
}

#[test]
fn sibling_criteria_that_enumerate_cases_are_not_lq_008() {
    let document = with_criteria(&[
        "The content address of the index is reproduced.",
        "The graph address of the index is reproduced.",
    ]);
    assert_eq!(
        of(&findings(&document), RuleId::Lq008),
        Vec::<&Finding>::new()
    );
}

#[test]
fn identical_sibling_criteria_are_lq_008() {
    let document = with_criteria(&["Nothing is written.", "Nothing is written!"]);
    let all = findings(&document);
    let found = of(&all, RuleId::Lq008);
    assert_eq!(found.len(), 1);
    assert_eq!(keys(found[0]), [key(70), key(71)]);
}

#[test]
fn identical_criteria_of_different_requirements_are_not_lq_008() {
    let mut document = with_criteria(&["Nothing is written."]);
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements.push(requirement(
        key(80),
        "If the reader fails, then the writer shall stop.",
        vec![criterion(key(81), "Nothing is written.")],
    ));
    assert_eq!(
        of(&findings(&document), RuleId::Lq008),
        Vec::<&Finding>::new()
    );
}

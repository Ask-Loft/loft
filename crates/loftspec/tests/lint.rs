//! The structural and v2 lint rules, findings and reports, and the lint configuration (D-11
//! F-19, F-21, F-22, F-23).

mod common;

use std::num::NonZeroU64;

use common::text::{FEATURE, files, path};
use common::{component, criterion, feature, key, product, requirement, root, small_document};
use loftspec::codec::{self, Problem, Sites};
use loftspec::document::{Document, Edge, TagEntry};
use loftspec::identity::{Key, LabelPrefix};
use loftspec::ids::{ComponentId, FeatureId, RequirementId, TagId};
use loftspec::lint::{Finding, LintConfig, RuleId, Severity, lint};
use loftspec::names::TagName;
use loftspec::statement::Statement;
use loftspec::tree::{Constraint, Details};

fn findings(document: &Document) -> Vec<Finding> {
    lint(document, &Sites::default(), &LintConfig::default())
}

fn configured(document: &Document, yaml: &str) -> Vec<Finding> {
    let config = LintConfig::read(Some(yaml.as_bytes())).unwrap();
    lint(document, &Sites::default(), &config)
}

/// The keys each finding of `rule` names.
fn of(findings: &[Finding], rule: RuleId) -> Vec<Vec<Key>> {
    findings
        .iter()
        .filter(|finding| finding.rule == rule)
        .map(|finding| finding.subjects.iter().map(|s| s.key).collect())
        .collect()
}

fn severity(rule: RuleId) -> Severity {
    LintConfig::default().severity(rule)
}

fn tag(name: &str) -> TagName {
    TagName::parse(name).unwrap()
}

fn constraint(n: u32) -> Constraint {
    Constraint {
        id: Some(loftspec::ids::ConstraintId::new(key(n))),
        name: "The part shall be bounded.".to_owned(),
        details: Details::default(),
        binding: None,
        conservation_kind: None,
        verification_fate: None,
    }
}

#[test]
fn ac_335_ac_337_a_feature_without_a_realizes_edge_is_sh_001() {
    let mut document = small_document();
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Realizes { .. }));
    assert_eq!(of(&findings(&document), RuleId::Sh001), [vec![key(3)]]);
    assert_eq!(severity(RuleId::Sh001), Severity::Warning);
}

#[test]
fn ac_336_a_feature_with_a_realizes_edge_is_not_sh_001() {
    assert_eq!(
        of(&findings(&small_document()), RuleId::Sh001),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_338_ac_341_a_leaf_component_no_feature_realizes_is_sh_002() {
    let mut document = small_document();
    document.components.push(root(component(key(40), "Unused")));
    assert_eq!(of(&findings(&document), RuleId::Sh002), [vec![key(40)]]);
    assert_eq!(severity(RuleId::Sh002), Severity::Info);
}

#[test]
fn ac_339_ac_340_a_component_with_a_child_or_realized_is_not_sh_002() {
    let mut document = small_document();
    let mut parent = component(key(40), "Parent");
    parent.components.push(component(key(41), "Child"));
    document.components.push(root(parent));
    document.edges.push(Edge::Realizes {
        feature: FeatureId::new(key(3)),
        component: ComponentId::new(key(41)),
    });
    assert_eq!(
        of(&findings(&document), RuleId::Sh002),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_342_ac_344_a_requirement_without_a_criterion_is_sh_003() {
    let mut document = small_document();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements[0].criteria.clear();
    assert_eq!(of(&findings(&document), RuleId::Sh003), [vec![key(4)]]);
    assert_eq!(severity(RuleId::Sh003), Severity::Warning);
}

#[test]
fn ac_343_ac_346_ac_350_ac_351_complete_parents_raise_no_structural_finding() {
    let all = findings(&small_document());
    for rule in [RuleId::Sh003, RuleId::Sh004, RuleId::Sh005] {
        assert!(of(&all, rule).is_empty(), "{rule}");
    }
}

#[test]
fn ac_345_ac_347_a_feature_without_a_requirement_is_sh_004() {
    let mut document = small_document();
    let group = &mut document.products[0].feature_groups[0];
    group.features.push(feature(key(42), "Empty", Vec::new()));
    assert_eq!(of(&findings(&document), RuleId::Sh004), [vec![key(42)]]);
    assert_eq!(severity(RuleId::Sh004), Severity::Warning);
}

#[test]
fn ac_348_ac_349_ac_352_a_product_or_feature_group_without_children_is_sh_005() {
    let mut document = small_document();
    document.products.push(product(key(43), "Bare"));
    document.products[0].feature_groups[0].features.clear();
    let mut found = of(&findings(&document), RuleId::Sh005);
    found.sort();
    assert_eq!(found, [vec![key(2)], vec![key(43)]]);
    assert_eq!(severity(RuleId::Sh005), Severity::Info);
}

#[test]
fn ac_353_ac_355_a_registry_entry_no_node_lists_is_sh_006() {
    let document = small_document();
    assert_eq!(of(&findings(&document), RuleId::Sh006), [vec![key(30)]]);
    assert_eq!(severity(RuleId::Sh006), Severity::Info);
}

#[test]
fn ac_354_a_registry_entry_a_node_lists_is_not_sh_006() {
    let mut document = small_document();
    document.components[0].details.tags.push(tag("identity"));
    assert_eq!(
        of(&findings(&document), RuleId::Sh006),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_356_ac_358_siblings_with_one_name_after_trimming_and_lowercasing_are_sh_007() {
    let mut document = small_document();
    let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
    requirement.criteria.push(criterion(
        key(44),
        " every NODE record carries a `key` field. ",
    ));
    assert_eq!(
        of(&findings(&document), RuleId::Sh007),
        [vec![key(5), key(44)]]
    );
    assert_eq!(severity(RuleId::Sh007), Severity::Warning);
}

#[test]
fn ac_871_a_constraint_named_like_a_sibling_feature_is_not_sh_007() {
    let mut document = small_document();
    let mut keys = constraint(49);
    keys.name = "Keys".to_owned();
    document.products[0].feature_groups[0]
        .constraints
        .push(keys);
    assert_eq!(
        of(&findings(&document), RuleId::Sh007),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_357_equal_names_under_different_parents_are_not_sh_007() {
    let mut document = small_document();
    document.components.push(root(component(key(45), "Keys")));
    assert_eq!(
        of(&findings(&document), RuleId::Sh007),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_359_ac_362_a_component_whose_subtree_holds_no_constraint_or_attribute_is_sh_008() {
    let mut document = small_document();
    document.components.push(root(component(key(46), "Loose")));
    assert_eq!(of(&findings(&document), RuleId::Sh008), [vec![key(46)]]);
    assert_eq!(severity(RuleId::Sh008), Severity::Info);
}

#[test]
fn ac_870_a_component_inside_a_subtree_that_yields_sh_008_yields_none_of_its_own() {
    let mut document = small_document();
    let mut parent = component(key(46), "Loose");
    parent.components.push(component(key(47), "Part"));
    document.components.push(root(parent));
    assert_eq!(of(&findings(&document), RuleId::Sh008), [vec![key(46)]]);
}

#[test]
fn ac_360_ac_361_a_constraint_or_attribute_in_the_subtree_clears_sh_008() {
    let mut document = small_document();
    let mut parent = component(key(46), "Bounded");
    let mut child = component(key(47), "Part");
    child.constraints.push(constraint(48));
    parent.components.push(child);
    document.components.push(root(parent));
    assert_eq!(
        of(&findings(&document), RuleId::Sh008),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_363_ac_365_a_tag_no_registry_entry_names_is_sh_009_naming_the_node_and_the_tag() {
    let mut document = small_document();
    document.components[0].details.tags.push(tag("unknown"));
    let all = findings(&document);
    let found: Vec<&Finding> = all.iter().filter(|f| f.rule == RuleId::Sh009).collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].subjects[0].key, key(6));
    assert_eq!(found[0].term.as_deref(), Some("unknown"));
    assert_eq!(severity(RuleId::Sh009), Severity::Warning);
}

#[test]
fn ac_364_a_tag_with_a_registry_entry_is_not_sh_009() {
    assert_eq!(
        of(&findings(&small_document()), RuleId::Sh009),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_366_to_ac_369_a_component_named_like_a_path_is_sh_010() {
    let mut document = small_document();
    for (n, name) in [
        (50, "src/model"),
        (51, "model\\types"),
        (52, "schema.rs"),
        (53, "Schema store"),
    ] {
        document.components.push(root(component(key(n), name)));
    }
    let mut found = of(&findings(&document), RuleId::Sh010);
    found.sort();
    assert_eq!(found, [vec![key(50)], vec![key(51)], vec![key(52)]]);
    assert_eq!(severity(RuleId::Sh010), Severity::Hint);
}

fn with_body(body: &str) -> Document {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some(body.to_owned());
    document
}

#[test]
fn ac_406_ac_408_each_heading_line_in_a_body_is_v2_001_naming_the_line() {
    let document = with_body("Intro.\n\n## Detail\n\nText.\nUnderlined\n----------\n");
    let all = findings(&document);
    let lines: Vec<Option<usize>> = all
        .iter()
        .filter(|f| f.rule == RuleId::V2001)
        .map(|f| f.body_line)
        .collect();
    assert_eq!(lines, [Some(3), Some(6)]);
    assert_eq!(severity(RuleId::V2001), Severity::Info);
}

#[test]
fn ac_407_a_body_without_a_heading_is_not_v2_001() {
    assert_eq!(
        of(&findings(&small_document()), RuleId::V2001),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_409_ac_411_a_statement_stored_as_text_is_v2_002() {
    let mut document = small_document();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements.push(requirement(
        key(54),
        "Keys should mostly work.",
        vec![criterion(key(55), "They work.")],
    ));
    assert_eq!(of(&findings(&document), RuleId::V2002), [vec![key(54)]]);
    assert_eq!(severity(RuleId::V2002), Severity::Info);
}

#[test]
fn ac_410_a_statement_with_a_form_is_not_v2_002() {
    let document = small_document();
    let statement = &document.products[0].feature_groups[0].features[0].requirements[0].statement;
    assert!(matches!(statement, Statement::Form(_)));
    assert_eq!(
        of(&findings(&document), RuleId::V2002),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_412_ac_414_ac_874_a_reference_without_a_mentions_edge_or_a_target_is_v2_003() {
    let mut document = with_body("See CMP-1 and RQ-99.");
    // RQ-99 was allocated once and its node has since gone.
    document
        .counters
        .set(LabelPrefix::Requirement, NonZeroU64::new(100).unwrap());
    let found: Vec<Finding> = findings(&document)
        .into_iter()
        .filter(|f| f.rule == RuleId::V2003)
        .collect();
    let terms: Vec<_> = found.iter().map(|f| f.term.clone().unwrap()).collect();
    assert_eq!(terms, ["CMP-1", "RQ-99"]);
    assert!(found.iter().all(|f| f.subjects[0].key == key(3)));
    assert_eq!(severity(RuleId::V2003), Severity::Warning);
}

#[test]
fn ac_890_a_label_shaped_text_at_or_above_its_counter_is_not_a_reference() {
    let mut document = with_body("ES256 with P-256, or P-2 at the counter.");
    document
        .counters
        .set(LabelPrefix::Product, NonZeroU64::new(2).unwrap());
    assert_eq!(
        of(&findings(&document), RuleId::V2003),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_413_a_reference_with_its_mentions_edge_is_not_v2_003() {
    let mut document = with_body("See CMP-1.");
    document.edges.push(Edge::Mentions {
        source: FeatureId::new(key(3)).into(),
        target: ComponentId::new(key(6)).into(),
    });
    assert_eq!(
        of(&findings(&document), RuleId::V2003),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_415_to_ac_419_a_finding_carries_rule_severity_keys_path_line_and_column() {
    let (document, sites) = codec::read_sited(&files()).unwrap();
    let all = lint(&document, &sites, &LintConfig::default());
    let finding = all.iter().find(|f| f.rule == RuleId::Sh006).unwrap();
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(finding.subjects[0].key, key(30));
    assert_eq!(
        finding.subjects[0].site.as_ref().unwrap().to_string(),
        "spec/tags.yaml:1:8"
    );
    assert_eq!(
        finding.to_string(),
        "spec/tags.yaml:1:8: info SH-006: Tag registry entry is applied by no node. `identity`"
    );
}

#[test]
fn ac_420_ac_421_findings_are_ordered_by_path_line_column_rule_and_keys_and_stable() {
    let mut files = files();
    let feature = String::from_utf8(files[&path(FEATURE)].clone()).unwrap();
    let edited = feature.replace("body: \"Keys address nodes.\"", "body: \"## Keys\"");
    files.insert(path(FEATURE), edited.into_bytes());
    let (document, sites) = codec::read_sited(&files).unwrap();
    let first = lint(&document, &sites, &LintConfig::default());
    let text: Vec<String> = first.iter().map(ToString::to_string).collect();
    let mut sorted = text.clone();
    sorted.sort();
    assert_eq!(text.len(), 3, "{text:#?}");
    assert_eq!(text, sorted);
    assert_eq!(lint(&document, &sites, &LintConfig::default()), first);
}

#[test]
fn ac_422_ac_866_a_rule_listed_under_rules_can_be_disabled() {
    let all = configured(
        &small_document(),
        "rules:\n  - rule: SH-006\n    enabled: false\n",
    );
    assert_eq!(of(&all, RuleId::Sh006), Vec::<Vec<Key>>::new());
}

#[test]
fn ac_423_a_rules_severity_can_be_set() {
    let all = configured(
        &small_document(),
        "rules:\n  - rule: SH-006\n    severity: warning\n",
    );
    let finding = all.iter().find(|f| f.rule == RuleId::Sh006).unwrap();
    assert_eq!(finding.severity, Severity::Warning);
}

#[test]
fn ac_425_ac_675_ac_867_a_suppression_names_a_rule_a_key_and_a_reason_and_drops_the_finding() {
    let yaml = format!(
        "suppressions:\n  - rule: SH-006\n    key: {}\n    reason: \"Kept for later.\"\n",
        key(30)
    );
    assert_eq!(
        of(&configured(&small_document(), &yaml), RuleId::Sh006),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn ac_426_without_a_lint_file_the_defaults_apply() {
    assert_eq!(LintConfig::read(None).unwrap(), LintConfig::default());
}

#[test]
fn ac_374_ac_389_ac_401_ac_647_to_ac_650_the_published_defaults_are_the_codes() {
    let published = include_str!("../../../testdata/golden/lint-defaults.yaml");
    assert_eq!(loftspec::lint::defaults_text(), published);
    let read = LintConfig::read(Some(published.as_bytes())).unwrap();
    assert_eq!(read, LintConfig::default());
}

#[test]
fn ac_672_ac_674_an_unknown_rule_is_an_error_at_its_line_and_column_named() {
    let errors =
        LintConfig::read(Some(b"rules:\n  - rule: SH-099\n    enabled: false\n")).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].to_string(),
        "spec/lint.yaml:2:11: `SH-099` is not a rule the Format defines"
    );
}

#[test]
fn ac_673_a_severity_outside_the_three_is_an_error_naming_the_value() {
    let errors =
        LintConfig::read(Some(b"rules:\n  - rule: SH-001\n    severity: error\n")).unwrap_err();
    assert_eq!(
        errors[0].to_string(),
        "spec/lint.yaml:3:15: `error` is not warning, info or hint"
    );
}

#[test]
fn ac_869_an_option_a_rule_does_not_take_is_an_error_at_its_line_and_column() {
    let errors = LintConfig::read(Some(
        b"rules:\n  - rule: SH-001\n    lexicon:\n      extend:\n        - a\n",
    ));
    let errors = errors.unwrap_err();
    assert_eq!(
        errors[0].problem,
        Problem::UnknownField("lexicon".to_owned())
    );
}

#[test]
fn a_suppression_without_a_reason_is_refused() {
    let yaml = format!("suppressions:\n  - rule: SH-006\n    key: {}\n", key(30));
    let errors = LintConfig::read(Some(yaml.as_bytes())).unwrap_err();
    assert_eq!(errors[0].problem, Problem::MissingField("reason"));
}

#[test]
fn every_rule_has_a_wire_name_and_a_default() {
    assert_eq!(RuleId::ALL.len(), 22);
    for rule in RuleId::ALL {
        assert_eq!(RuleId::parse(rule.as_str()), Some(*rule));
    }
    assert!(!RuleId::Lq004.enabled_by_default());
    assert!(!RuleId::Lq007.enabled_by_default());
}

#[test]
fn a_tag_registry_entry_key_is_its_findings_subject() {
    let mut document = small_document();
    document.tags.push(TagEntry {
        id: TagId::new(key(60)),
        name: tag("spare"),
        definition: "Unused.".to_owned(),
    });
    let mut found = of(&findings(&document), RuleId::Sh006);
    found.sort();
    assert_eq!(found, [vec![key(30)], vec![key(60)]]);
    let _ = RequirementId::new(key(4));
}

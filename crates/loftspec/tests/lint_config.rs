//! The lint configuration (F-23): `spec/lint.yaml` turns rules off, sets their severities and
//! options and suppresses findings, each error in it named at its line and column.

use crate::common::lint::{configured, keys_of};
use crate::common::{key, small_document};
use loftspec::codec::Problem;
use loftspec::identity::Key;
use loftspec::lint::{LintConfig, RuleId, Severity};

#[test]
fn a_rule_listed_under_rules_can_be_disabled() {
    let all = configured(
        &small_document(),
        "rules:\n  - rule: SH-006\n    enabled: false\n",
    );
    assert_eq!(keys_of(&all, RuleId::Sh006), Vec::<Vec<Key>>::new());
}

#[test]
fn a_rules_severity_can_be_set() {
    let all = configured(
        &small_document(),
        "rules:\n  - rule: SH-006\n    severity: warning\n",
    );
    let finding = all.iter().find(|f| f.rule == RuleId::Sh006).unwrap();
    assert_eq!(finding.severity, Severity::Warning);
}

#[test]
fn a_suppression_names_a_rule_a_key_and_a_reason_and_drops_the_finding() {
    let yaml = format!(
        "suppressions:\n  - rule: SH-006\n    key: {}\n    reason: \"Kept for later.\"\n",
        key(30)
    );
    assert_eq!(
        keys_of(&configured(&small_document(), &yaml), RuleId::Sh006),
        Vec::<Vec<Key>>::new()
    );
}

#[test]
fn without_a_lint_file_the_defaults_apply() {
    assert_eq!(LintConfig::read(None).unwrap(), LintConfig::default());
}

#[test]
fn the_published_defaults_are_the_codes() {
    let published = include_str!("../../../testdata/golden/lint-defaults.yaml");
    assert_eq!(loftspec::lint::defaults_text(), published);
    let read = LintConfig::read(Some(published.as_bytes())).unwrap();
    assert_eq!(read, LintConfig::default());
}

#[test]
fn an_unknown_rule_is_an_error_at_its_line_and_column_named() {
    let errors =
        LintConfig::read(Some(b"rules:\n  - rule: SH-099\n    enabled: false\n")).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].to_string(),
        "spec/lint.yaml:2:11: `SH-099` is not a rule the Format defines"
    );
}

#[test]
fn a_severity_outside_the_three_is_an_error_naming_the_value() {
    let errors =
        LintConfig::read(Some(b"rules:\n  - rule: SH-001\n    severity: error\n")).unwrap_err();
    assert_eq!(
        errors[0].to_string(),
        "spec/lint.yaml:3:15: `error` is not warning, info or hint"
    );
}

#[test]
fn an_option_a_rule_does_not_take_is_an_error_at_its_line_and_column() {
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
    assert_eq!(RuleId::ALL.len(), 30);
    for rule in RuleId::ALL {
        assert_eq!(RuleId::parse(rule.as_str()), Some(*rule));
    }
    assert!(!RuleId::Lq004.enabled_by_default());
    assert!(!RuleId::Lq007.enabled_by_default());
}

#[test]
fn a_list_written_with_no_value_is_empty() {
    let empty_rules = LintConfig::read(Some(b"rules:\nsuppressions:\n")).unwrap();
    assert_eq!(empty_rules, LintConfig::default());
    let yaml = "rules:\n  - rule: LQ-001\n    lexicon:\n      replace:\n";
    let config = LintConfig::read(Some(yaml.as_bytes())).unwrap();
    assert_eq!(config.lexicon(RuleId::Lq001), &Vec::<Vec<String>>::new());
}

#[test]
fn the_published_defaults_state_every_option() {
    let published = include_str!("../../../testdata/golden/lint-defaults.yaml");
    for option in [
        "lexicon",
        "modalLexicon",
        "conjunctionLexicon",
        "conjunctionThreshold",
        "minTokens",
        "threshold",
        "maxCriteria",
        "maxBodyWords",
        "properNames",
    ] {
        assert!(published.contains(&format!("    {option}:")), "{option}");
    }
}

#[test]
fn a_count_below_its_least_value_is_an_error_naming_the_value() {
    for (rule, option, least) in [
        ("LQ-005", "conjunctionThreshold", 2),
        ("LQ-008", "minTokens", 1),
        ("V2-006", "maxCriteria", 1),
        ("V2-006", "maxBodyWords", 1),
    ] {
        let yaml = |value: usize| format!("rules:\n  - rule: {rule}\n    {option}: {value}\n");
        assert!(
            LintConfig::read(Some(yaml(least).as_bytes())).is_ok(),
            "{option}"
        );
        let errors = LintConfig::read(Some(yaml(least - 1).as_bytes())).unwrap_err();
        let message = errors[0].to_string();
        assert!(message.contains(&format!("at least {least}")), "{message}");
    }
}

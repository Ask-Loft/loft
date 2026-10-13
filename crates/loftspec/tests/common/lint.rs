//! Linting a document in a test, and reading its findings: shared by the lint suites.

use loftspec::codec::Sites;
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::lint::{Finding, LintConfig, RuleId, Severity, lint};
use loftspec::tree::Requirement;

/// The findings of the default configuration.
pub fn findings(document: &Document) -> Vec<Finding> {
    lint(document, &Sites::default(), &LintConfig::default())
}

/// The findings of the configuration that `yaml` states.
pub fn configured(document: &Document, yaml: &str) -> Vec<Finding> {
    let config = LintConfig::read(Some(yaml.as_bytes())).unwrap();
    lint(document, &Sites::default(), &config)
}

/// The findings of `rule`.
pub fn of(findings: &[Finding], rule: RuleId) -> Vec<&Finding> {
    findings
        .iter()
        .filter(|finding| finding.rule == rule)
        .collect()
}

/// The keys a finding names.
pub fn keys(finding: &Finding) -> Vec<Key> {
    finding.subjects.iter().map(|subject| subject.key).collect()
}

/// The default severity of `rule`.
pub fn severity(rule: RuleId) -> Severity {
    LintConfig::default().severity(rule)
}

/// The Requirement a test added after the small document's first, under F-1.
pub fn added(document: &mut Document) -> &mut Requirement {
    &mut document.products[0].feature_groups[0].features[0].requirements[1]
}

//! The findings about one written `spec.md` beyond its requirements' text: content a second
//! import would read differently (`OR-007`), `ext.openspec` values the renderer cannot read
//! (`OR-008`), a placeholder Purpose (`OR-009`), a Feature with no Requirement (`OR-010`) and
//! Sections whose headings `OpenSpec` reads as something else (`OR-011`, `OR-012`).

use std::collections::BTreeSet;

use super::findings::{Finding, Rule};
use super::spec::Spec;
use crate::convert::openspec::ext::{unreadable_feature, unreadable_requirement};
use crate::convert::openspec::placeholder::is_placeholder;
use crate::ids::FeatureId;
use crate::tree::Feature;

/// The words that, before `Requirements`, name a delta section.
const DELTAS: [&str; 4] = ["added", "modified", "removed", "renamed"];

/// The opening that makes a level-3 heading a requirement heading.
const REQUIREMENT: &str = "requirement:";

pub(super) fn findings(feature: &Feature, spec: &Spec, path: &str) -> Vec<Finding> {
    let at = |rule, line, key| Finding::at(rule, path.to_owned(), line, key);
    let feature_key = feature.id.map(FeatureId::key);
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for unread in &spec.unread {
        if seen.insert((unread.key, unread.kind)) {
            let detail = Some(unread.kind.as_str().to_owned());
            findings.push(at(Rule::ReadsBack, unread.line, unread.key).with_detail(detail));
        }
    }
    for field in unreadable_feature(&feature.details.ext) {
        findings.push(at(Rule::UnreadableExt, 1, feature_key).with_detail(Some(field)));
    }
    for (requirement, written) in feature.requirements.iter().zip(&spec.requirements) {
        for field in unreadable_requirement(&requirement.details.ext) {
            let finding = at(Rule::UnreadableExt, written.line, written.key);
            findings.push(finding.with_detail(Some(field)));
        }
    }
    if is_placeholder(&spec.purpose) {
        findings.push(at(Rule::PlaceholderPurpose, spec.purpose_line, feature_key));
    }
    if feature.requirements.is_empty() {
        findings.push(at(Rule::NoRequirement, 1, feature_key));
    }
    for section in &spec.sections {
        if section.level == 3 && reads_as_requirement(&section.name) {
            findings.push(at(Rule::SectionAsRequirement, section.line, section.key));
        }
        if section.level == 2 && is_delta(&section.name) {
            findings.push(at(Rule::DeltaSection, section.line, section.key));
        }
    }
    findings
}

/// Whether a name opens with `Requirement:` in any case and has text after it.
fn reads_as_requirement(name: &str) -> bool {
    let name = name.trim_start();
    name.get(..REQUIREMENT.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(REQUIREMENT))
        && name.len() > REQUIREMENT.len()
}

/// Whether a name is a delta word and `Requirements`, in any case.
fn is_delta(name: &str) -> bool {
    let words: Vec<&str> = name.split_whitespace().collect();
    matches!(words.as_slice(), [delta, requirements]
        if DELTAS.iter().any(|word| delta.eq_ignore_ascii_case(word))
            && requirements.eq_ignore_ascii_case("requirements"))
}

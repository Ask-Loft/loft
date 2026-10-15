//! The `OpenSpec` converter's findings: one code per rule, each naming its file and line where it
//! has one and the key of the node it concerns where there is one, and none for a faithful
//! project.

use crate::common::openspec::{AUTH, converted, one, project};
use loftspec::convert::openspec::{Conversion, Finding, Project, Rule};
use loftspec::identity::Key;

fn of(conversion: &Conversion, rule: Rule) -> Vec<Finding> {
    conversion
        .findings
        .iter()
        .filter(|finding| finding.rule == rule)
        .cloned()
        .collect()
}

/// The one finding of `rule`, with its file, line and whether it names a key.
fn site(conversion: &Conversion, rule: Rule) -> (Option<String>, Option<usize>, Option<Key>) {
    let findings = of(conversion, rule);
    let [finding] = findings.as_slice() else {
        panic!(
            "one finding of {}: {:?}",
            rule.as_str(),
            conversion.findings
        );
    };
    (finding.file.clone(), finding.line, finding.key)
}

/// The key the sidecar records for the node of `kind` named `name`.
fn key(conversion: &Conversion, kind: &str, name: &str) -> Key {
    conversion
        .sidecar
        .nodes
        .iter()
        .find(|node| node.kind == kind && node.name.as_deref() == Some(name))
        .unwrap()
        .key
}

fn file() -> String {
    "specs/auth/spec.md".to_owned()
}

#[test]
fn a_faithful_project_has_no_finding_but_the_count_of_rewritten_statements() {
    let conversion = converted(&one(AUTH));
    let codes: Vec<&str> = conversion
        .findings
        .iter()
        .map(|finding| finding.rule.as_str())
        .collect();
    assert_eq!(codes, ["OS-008"]);
}

#[test]
fn every_rule_has_its_own_code() {
    let codes: Vec<&str> = Rule::ALL.iter().map(|rule| rule.as_str()).collect();
    // OS-009 reported metadata kept in a body; metadata now goes to `ext`, and the code is
    // retired, never reused.
    let expected: Vec<String> = (1..=14)
        .filter(|n| *n != 9)
        .map(|n| format!("OS-{n:03}"))
        .collect();
    assert_eq!(codes, expected);
}

#[test]
fn skipped_front_matter_is_reported_at_the_line_that_closes_it() {
    let conversion = converted(&one(&format!(
        "---\nstatus: done\nretrofit: true\n---\n{AUTH}"
    )));
    assert_eq!(
        site(&conversion, Rule::FrontMatter),
        (Some(file()), Some(4), None)
    );
}

#[test]
fn a_placeholder_purpose_is_reported_at_its_heading_with_the_features_key() {
    let text = "# x\n\n## Purpose\nTBD - created by archiving change add-x. Update Purpose after archive.\n";
    let conversion = converted(&one(text));
    let feature = key(&conversion, "feature", "x");
    assert_eq!(
        site(&conversion, Rule::PlaceholderPurpose),
        (Some(file()), Some(3), Some(feature))
    );
}

#[test]
fn another_level_two_section_and_an_introduction_are_reported_with_their_keys() {
    let text = "# Auth\nPreface.\n\n## Purpose\nP.\n\n## Notes\nN.\n";
    let conversion = converted(&one(text));
    let findings: Vec<_> = of(&conversion, Rule::ExtraSection)
        .iter()
        .map(|finding| (finding.line, finding.key))
        .collect();
    assert_eq!(
        findings,
        [
            (Some(2), Some(key(&conversion, "section", "Introduction"))),
            (Some(7), Some(key(&conversion, "section", "Notes")))
        ]
    );
    assert_eq!(of(&converted(&one(AUTH)), Rule::ExtraSection), []);
}

#[test]
fn text_of_requirements_outside_every_requirement_is_reported_with_its_section() {
    let text = "## Requirements\nRead in order.\n\n### Glossary\nTerms.\n";
    let conversion = converted(&one(text));
    let lines: Vec<_> = of(&conversion, Rule::StrayRequirementsText)
        .iter()
        .map(|finding| (finding.line, finding.key))
        .collect();
    assert_eq!(
        lines,
        [
            (Some(2), Some(key(&conversion, "section", "Requirements"))),
            (Some(4), Some(key(&conversion, "section", "Glossary")))
        ]
    );
}

#[test]
fn a_child_that_is_not_a_scenario_is_reported_with_its_section() {
    let text = "## Requirements\n\n### Requirement: R\nThe system SHALL r.\n\n#### Notes\nN.\n";
    let conversion = converted(&one(text));
    let section = key(&conversion, "section", "Notes");
    assert_eq!(
        site(&conversion, Rule::NonScenarioChild),
        (Some(file()), Some(6), Some(section))
    );
}

#[test]
fn a_delta_section_is_reported_as_one_and_not_as_another_section() {
    let text = "## Purpose\nP.\n\n## added   REQUIREMENTS\n\n### Requirement: New\nThe system SHALL be new.\n";
    let conversion = converted(&one(text));
    let section = key(&conversion, "section", "added   REQUIREMENTS");
    assert_eq!(
        site(&conversion, Rule::DeltaSection),
        (Some(file()), Some(4), Some(section))
    );
    assert_eq!(of(&conversion, Rule::ExtraSection), []);
}

#[test]
fn a_requirement_without_an_upper_case_modal_word_is_reported() {
    let text = "## Requirements\n\n### Requirement: Quiet\nThe system should stay quiet.\n\n### Requirement: Lower\nThe system must log.\n\n### Requirement: Loud\nThe system SHALL log.\n";
    let conversion = converted(&one(text));
    let reported: Vec<_> = of(&conversion, Rule::NoModal)
        .iter()
        .map(|finding| (finding.line, finding.key))
        .collect();
    assert_eq!(
        reported,
        [
            (Some(3), Some(key(&conversion, "requirement", "Quiet"))),
            (Some(6), Some(key(&conversion, "requirement", "Lower")))
        ]
    );
}

#[test]
fn the_number_of_rewritten_statements_is_reported_once() {
    let conversion = converted(&project(&[("a/spec.md", AUTH), ("b/spec.md", AUTH)]));
    let findings = of(&conversion, Rule::ModalRewritten);
    let [finding] = findings.as_slice() else {
        panic!("one finding");
    };
    assert_eq!(finding.detail.as_deref(), Some("4"));
    assert_eq!((finding.file.as_deref(), finding.key), (None, None));
    let verbatim = "## Requirements\n\n### Requirement: R\nThe system shall r.\n";
    assert_eq!(of(&converted(&one(verbatim)), Rule::ModalRewritten), []);
}

#[test]
fn a_second_requirement_of_one_name_is_reported() {
    let text = "## Requirements\n\n### Requirement: Twin\nThe system SHALL a.\n\n### Requirement: Twin\nThe system SHALL b.\n";
    let conversion = converted(&one(text));
    let second = conversion
        .sidecar
        .nodes
        .iter()
        .filter(|node| node.kind == "requirement")
        .nth(1)
        .unwrap()
        .key;
    assert_eq!(
        site(&conversion, Rule::DuplicateRequirement),
        (Some(file()), Some(6), Some(second))
    );
}

#[test]
fn active_and_archived_changes_are_reported_with_their_counts() {
    let project = Project {
        active_changes: vec!["add-a".to_owned(), "fix-b".to_owned()],
        archived_changes: 7,
        ..one(AUTH)
    };
    let conversion = converted(&project);
    let detail = |rule| of(&conversion, rule)[0].detail.clone();
    assert_eq!(
        detail(Rule::ActiveChanges).as_deref(),
        Some("2 (add-a, fix-b)")
    );
    assert_eq!(detail(Rule::ArchivedChanges).as_deref(), Some("7"));
    let quiet = converted(&one(AUTH));
    assert_eq!(of(&quiet, Rule::ActiveChanges), []);
    assert_eq!(of(&quiet, Rule::ArchivedChanges), []);
}

#[test]
fn the_context_and_store_of_config_yaml_are_reported_at_their_lines() {
    let project = Project {
        config_yaml: Some(
            "schema: spec-driven\ncontext: |\n  Rust project.\nstore: ../plans\n".to_owned(),
        ),
        ..one(AUTH)
    };
    let conversion = converted(&project);
    let config = || Some("config.yaml".to_owned());
    assert_eq!(
        site(&conversion, Rule::ConfigContext),
        (config(), Some(2), None)
    );
    assert_eq!(site(&conversion, Rule::Store), (config(), Some(4), None));
    let plain = Project {
        config_yaml: Some("schema: spec-driven\n".to_owned()),
        ..one(AUTH)
    };
    assert_eq!(of(&converted(&plain), Rule::ConfigContext), []);
}

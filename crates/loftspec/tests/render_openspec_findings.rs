//! What the `OpenSpec` renderer reports: what `OpenSpec`'s validator would reject or warn about,
//! and the content the projection leaves out. Every finding is info and none stops the render.

use loftspec::document::Document;
use loftspec::render::openspec::{Finding, Rule, openspec};
use loftspec::statement::Statement;

use crate::common::full::full_document;
use crate::common::openspec::{converted, one, project};
use crate::common::{key, small_document};

/// A Purpose long enough to raise no `OR-003`.
const PURPOSE: &str = "## Purpose\nA purpose long enough that OpenSpec does not warn about it.\n\n";

fn findings(document: &Document) -> Vec<Finding> {
    openspec(document).unwrap().findings
}

fn of(document: &Document, rule: Rule) -> Vec<Finding> {
    findings(document)
        .into_iter()
        .filter(|finding| finding.rule == rule)
        .collect()
}

/// The findings of `rule` for one spec holding the given Requirements text.
fn of_spec(requirements: &str, rule: Rule) -> Vec<Finding> {
    let text = format!("{PURPOSE}## Requirements\n\n{requirements}");
    of(&converted(&one(&text)).document, rule)
}

#[test]
fn a_document_renders_whatever_it_raises_and_each_finding_is_info() {
    let document = full_document();
    let projection = openspec(&document).unwrap();
    assert_ne!(projection.files.len(), 0);
    assert_ne!(projection.findings.len(), 0);
    for finding in &projection.findings {
        assert!(
            finding.to_string().contains(" info OR-")
                || finding.to_string().starts_with("info OR-"),
            "{finding}"
        );
    }
}

#[test]
fn a_finding_about_a_node_names_its_key_and_its_file() {
    let finding = &of(&small_document(), Rule::ShortPurpose)[0];
    assert_eq!(finding.key, Some(key(3)));
    assert_eq!(finding.file.as_deref(), Some("specs/identity/keys/spec.md"));
    assert!(
        finding
            .to_string()
            .starts_with("specs/identity/keys/spec.md:")
    );
}

#[test]
fn a_requirement_text_without_shall_or_must_as_a_word_raises_or_001() {
    let quiet = "### Requirement: Quiet\nThe system keeps a log, SHALLOW or MUSTY.\n\n#### Scenario: S\n- **THEN** logged\n";
    assert_eq!(of_spec(quiet, Rule::NoModal).len(), 1);
    let loud = "### Requirement: Loud\nThe system SHALL keep a log.\n\n#### Scenario: S\n- **THEN** logged\n";
    assert_eq!(of_spec(loud, Rule::NoModal), []);
}

#[test]
fn the_requirement_text_leaves_out_blank_lines_fenced_code_and_metadata_lines() {
    let mut document = small_document();
    let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
    requirement.statement = Statement::Text("Keys are written.".to_owned());
    requirement.details.body = Some("```\nThe writer SHALL\n```\n\n**Rule**: MUST".to_owned());
    assert_eq!(of(&document, Rule::NoModal).len(), 1);
    let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
    let tilde_inside = "````\n~~~\nThe writer SHALL\n```\nMUST\n````";
    requirement.details.body = Some(tilde_inside.to_owned());
    assert_eq!(
        of(&document, Rule::NoModal).len(),
        1,
        "one fence holds it all"
    );
    let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
    requirement.details.body = Some("Keys MUST be unique.".to_owned());
    assert_eq!(of(&document, Rule::NoModal), []);
}

#[test]
fn a_requirement_without_a_scenario_with_text_raises_or_002() {
    let none = "### Requirement: Bare\nThe system SHALL r.\n";
    assert_eq!(of_spec(none, Rule::NoScenario).len(), 1);
    let one_scenario = "### Requirement: R\nThe system SHALL r.\n\n#### Scenario: S\n";
    assert_eq!(
        of_spec(one_scenario, Rule::NoScenario),
        [],
        "a criterion gets a THEN line"
    );
    let empty_section = "### Requirement: R\nThe system SHALL r.\n\n#### Notes\n";
    assert_eq!(of_spec(empty_section, Rule::NoScenario).len(), 1);
    let section = "### Requirement: R\nThe system SHALL r.\n\n#### Notes\nText.\n";
    assert_eq!(of_spec(section, Rule::NoScenario), []);
}

#[test]
fn a_purpose_absent_or_under_fifty_characters_raises_or_003_with_its_length() {
    let short = &of(&small_document(), Rule::ShortPurpose)[0];
    assert_eq!(short.detail.as_deref(), Some("19 characters"));
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0]
        .details
        .body = None;
    let absent = &of(&document, Rule::ShortPurpose)[0];
    assert_eq!(absent.detail.as_deref(), Some("0 characters"));
    let body = format!("  {}  ", "x".repeat(50));
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some(body);
    assert_eq!(of(&document, Rule::ShortPurpose), []);
}

#[test]
fn a_requirement_text_over_500_characters_raises_or_004() {
    let at = |length: usize| {
        let mut document = small_document();
        let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
        let statement = format!("The writer SHALL {}", "k".repeat(length - 17 - 2));
        requirement.statement = Statement::Text(statement);
        requirement.details.body = Some("   a\n".to_owned());
        of(&document, Rule::LongRequirement)
    };
    assert_eq!(at(500), []);
    let long = at(501);
    assert_eq!(long.len(), 1);
    assert_eq!(long[0].detail.as_deref(), Some("501 characters"));
}

#[test]
fn each_kind_of_content_left_out_is_counted_once_and_an_absent_kind_raises_nothing() {
    let details: Vec<String> = of(&full_document(), Rule::LeftOut)
        .into_iter()
        .map(|finding| finding.detail.unwrap())
        .collect();
    let kinds: Vec<&str> = details
        .iter()
        .map(|detail| detail.split_once(' ').unwrap().1)
        .collect();
    assert_eq!(
        kinds,
        [
            "Components",
            "Constraints",
            "Quality Attributes",
            "Decisions",
            "edges",
            "tags"
        ]
    );
    let document = full_document();
    assert_eq!(
        details[3],
        format!("{} Decisions", document.decisions.len())
    );
    assert_eq!(details[4], format!("{} edges", document.edges.len()));
    let bare = converted(&one(&format!("{PURPOSE}## Requirements\n"))).document;
    assert_eq!(of(&bare, Rule::LeftOut), []);
}

#[test]
fn a_repeated_requirement_name_within_a_spec_raises_or_006_and_across_specs_nothing() {
    let twin = "### Requirement: Twin\nThe system SHALL a.\n\n#### Scenario: S\n- **THEN** a\n\n";
    let repeated = of_spec(&twin.repeat(2), Rule::DuplicateName);
    assert_eq!(repeated.len(), 1);
    assert_eq!(repeated[0].detail.as_deref(), Some("Twin"));
    let spec = format!("{PURPOSE}## Requirements\n\n{twin}");
    let apart = converted(&project(&[("a/spec.md", &spec), ("b/spec.md", &spec)]));
    assert_eq!(of(&apart.document, Rule::DuplicateName), []);
}

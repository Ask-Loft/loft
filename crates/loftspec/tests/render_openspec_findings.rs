//! What the `OpenSpec` renderer reports: what `OpenSpec`'s validator would reject or warn about,
//! and the content the projection leaves out. Every finding is info and none stops the render.

use std::collections::BTreeMap;

use loftspec::document::Document;
use loftspec::ext::ExtValue;
use loftspec::names::Namespace;
use loftspec::render::openspec::{Finding, Rule, openspec};
use loftspec::statement::Statement;

use crate::common::full::full_document;
use crate::common::openspec::{converted, one, project};
use crate::common::{key, section, small_document};

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

/// Sets `field` of `ext.openspec` to `value`.
fn put(ext: &mut loftspec::ext::Ext, field: &str, value: ExtValue) {
    let namespace = Namespace::parse("openspec").unwrap();
    let entry = ext
        .entry(namespace)
        .or_insert_with(|| ExtValue::Map(BTreeMap::new()));
    let ExtValue::Map(map) = entry else {
        panic!("a mapping")
    };
    map.insert(field.to_owned(), value);
}

fn text(value: &str) -> ExtValue {
    ExtValue::Text(value.to_owned())
}

fn first_requirement(document: &mut Document) -> &mut loftspec::tree::Requirement {
    &mut document.products[0].feature_groups[0].features[0].requirements[0]
}

fn first_feature(document: &mut Document) -> &mut loftspec::tree::Feature {
    &mut document.products[0].feature_groups[0].features[0]
}

fn details(findings: Vec<Finding>) -> Vec<String> {
    findings
        .into_iter()
        .map(|finding| finding.detail.unwrap_or_default())
        .collect()
}

#[test]
fn a_requirement_text_of_metadata_lines_alone_is_read_for_shall_or_must() {
    let mut document = small_document();
    let requirement = first_requirement(&mut document);
    requirement.statement = Statement::Text(String::new());
    let entry = ExtValue::Map(BTreeMap::from([
        ("label".to_owned(), text("Rule")),
        ("value".to_owned(), text("The writer SHALL write keys.")),
    ]));
    put(
        &mut requirement.details.ext,
        "metadata",
        ExtValue::List(vec![entry]),
    );
    assert_eq!(of(&document, Rule::NoModal), []);
}

#[test]
fn the_requirement_text_ends_at_a_heading_of_any_level() {
    let mut document = small_document();
    let requirement = first_requirement(&mut document);
    requirement.statement = Statement::Text("Keys are written.".to_owned());
    requirement.details.body = Some("##### Deeper\nThe writer SHALL write keys.".to_owned());
    assert_eq!(of(&document, Rule::NoModal).len(), 1);
}

#[test]
fn a_level_four_section_whose_own_section_has_text_counts_as_a_scenario() {
    let nested =
        "### Requirement: R\nThe system SHALL r.\n\n#### Notes\n\n##### Detail\nMore detail.\n";
    assert_eq!(of_spec(nested, Rule::NoScenario), []);
}

#[test]
fn names_that_differ_by_a_closing_run_of_hashes_or_spaces_are_the_same_name() {
    let twin = "### Requirement: Twin\nThe system SHALL a.\n\n#### Scenario: S\n- **THEN** a\n\n";
    let mut document = converted(&one(&format!(
        "{PURPOSE}## Requirements\n\n{}",
        twin.repeat(2)
    )))
    .document;
    let second = &mut document.products[0].feature_groups[0].features[0].requirements[1];
    put(&mut second.details.ext, "name", text("Twin ## "));
    let repeated = of(&document, Rule::DuplicateName);
    assert_eq!(repeated.len(), 1);
}

#[test]
fn content_a_second_import_would_read_differently_is_reported_once_per_node_and_kind() {
    assert_eq!(of(&small_document(), Rule::ReadsBack), []);
    let mut document = small_document();
    let feature = first_feature(&mut document);
    feature.details.body = Some("Keys.\n### Not a heading here\n#### Nor here".to_owned());
    let requirement = &mut feature.requirements[0];
    requirement.details.body = Some("**Note**: one\n\n```\n**Fenced**: no\n```".to_owned());
    requirement.criteria[0].name = "Two\nlines".to_owned();
    let mut inner = section(76, "Depth 6", "");
    for depth in (1..6).rev() {
        let mut outer = section(70 + depth, &format!("Depth {depth}"), "");
        outer.sections.push(inner);
        inner = outer;
    }
    feature.details.sections = vec![inner];
    let mut found = details(of(&document, Rule::ReadsBack));
    found.sort();
    assert_eq!(
        found,
        [
            "a Requirement body line that opens with a bold label and a colon",
            "a Section deeper than level 6",
            "a body line that is a heading",
            "a name or a text statement that holds a line break",
        ]
    );
}

#[test]
fn each_ext_openspec_value_the_renderer_cannot_read_is_reported_by_its_field() {
    assert_eq!(of(&small_document(), Rule::UnreadableExt), []);
    let mut document = small_document();
    let feature = first_feature(&mut document);
    put(&mut feature.details.ext, "capability", ExtValue::Integer(1));
    put(&mut feature.details.ext, "purpose", ExtValue::Boolean(true));
    let requirement = &mut feature.requirements[0];
    put(&mut requirement.details.ext, "name", ExtValue::Integer(2));
    put(&mut requirement.details.ext, "modal", text("may"));
    put(&mut requirement.details.ext, "metadata", text("ID: 1"));
    let unreadable = of(&document, Rule::UnreadableExt);
    assert!(unreadable.iter().all(|finding| finding.key.is_some()));
    let mut found = details(unreadable);
    found.sort();
    assert_eq!(
        found,
        [
            "ext.openspec.capability",
            "ext.openspec.metadata",
            "ext.openspec.modal",
            "ext.openspec.name",
            "ext.openspec.purpose",
        ]
    );
    let mut document = small_document();
    let feature = first_feature(&mut document);
    put(&mut feature.details.ext, "capability", text("../out"));
    let entry = ExtValue::Map(BTreeMap::from([("label".to_owned(), text("ID"))]));
    let requirement = &mut feature.requirements[0];
    put(
        &mut requirement.details.ext,
        "metadata",
        ExtValue::List(vec![entry]),
    );
    let mut found = details(of(&document, Rule::UnreadableExt));
    found.sort();
    assert_eq!(found, ["ext.openspec.capability", "ext.openspec.metadata"]);
}

#[test]
fn a_purpose_that_opens_with_a_tbd_or_todo_marker_or_holds_the_archive_sentence_raises_or_009() {
    let raised = |purpose: &str| {
        let mut document = small_document();
        first_feature(&mut document).details.body = Some(purpose.to_owned());
        of(&document, Rule::PlaceholderPurpose).len()
    };
    for placeholder in [
        "TBD - created by syncing change x.",
        "TODO write this",
        "tbd: later",
        "Todo.",
        "  TBD",
        "Intro.\nTBD - created by archiving change add-x. Update Purpose after archive.",
    ] {
        assert_eq!(raised(placeholder), 1, "{placeholder:?}");
    }
    for purpose in [
        "TBDs are listed in the backlog.",
        "todo items are tracked",
        "Keys address nodes.",
    ] {
        assert_eq!(raised(purpose), 0, "{purpose:?}");
    }
}

#[test]
fn a_feature_with_no_requirement_raises_or_010() {
    assert_eq!(of(&small_document(), Rule::NoRequirement), []);
    let bare = converted(&one(&format!("{PURPOSE}## Requirements\n"))).document;
    let found = of(&bare, Rule::NoRequirement);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].file.as_deref(), Some("specs/auth/spec.md"));
    assert!(found[0].key.is_some());
}

#[test]
fn a_level_three_section_opening_with_requirement_raises_or_011() {
    let mut document = small_document();
    let mut outer = section(60, "Requirement: Top", "Text.");
    outer
        .sections
        .push(section(61, "requirement: Ghost", "Text."));
    first_feature(&mut document).details.sections = vec![outer];
    let found = of(&document, Rule::SectionAsRequirement);
    assert_eq!(found.len(), 1, "only the level-3 one");
    assert_eq!(found[0].key, Some(key(61)));
}

#[test]
fn a_level_two_section_named_as_a_delta_section_raises_or_012() {
    let mut document = small_document();
    let mut outer = section(60, "added   Requirements", "Text.");
    outer
        .sections
        .push(section(61, "REMOVED Requirements", "Text."));
    first_feature(&mut document).details.sections = vec![outer];
    let found = of(&document, Rule::DeltaSection);
    assert_eq!(found.len(), 1, "only the level-2 one");
    assert_eq!(found[0].key, Some(key(60)));
}

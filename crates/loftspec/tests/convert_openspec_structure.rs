//! The `OpenSpec` converter's structure mapping: one Product, a Feature Group per first path
//! segment, a Feature per capability with its name, body and Sections, and Requirements,
//! Criteria and Sections in source order.

use crate::common::openspec::{AUTH, converted, features, one, openspec, project, requirements};
use loftspec::convert::openspec::Project;
use loftspec::ext::ExtValue;
use loftspec::tree::Section;

fn names(sections: &[Section]) -> Vec<&str> {
    sections
        .iter()
        .map(|section| section.name.as_str())
        .collect()
}

#[test]
fn a_project_becomes_one_product_with_no_architecture_or_decisions() {
    let document = converted(&one(AUTH)).document;
    let [product] = document.products.as_slice() else {
        panic!("one Product");
    };
    assert_eq!(product.name, "Shop");
    assert_eq!(product.slug.as_str(), "shop");
    assert_eq!(document.components, []);
    assert_eq!(product.constraints, []);
    assert_eq!(document.decisions, []);
    let feature = &product.feature_groups[0].features[0];
    assert_eq!(feature.quality_attributes, []);
}

#[test]
fn a_legacy_project_md_becomes_the_products_section_project_verbatim() {
    let project = Project {
        project_md: Some("# Project Context\n\n## Tech Stack\n- Rust\n".to_owned()),
        ..one(AUTH)
    };
    let document = converted(&project).document;
    let [section] = document.products[0].details.sections.as_slice() else {
        panic!("one Section");
    };
    assert_eq!(section.name, "Project");
    assert_eq!(section.body, "# Project Context\n\n## Tech Stack\n- Rust");
}

#[test]
fn capabilities_are_grouped_by_their_first_segment_and_flat_ones_in_capabilities() {
    let document = converted(&project(&[
        ("user_profiles/avatars/spec.md", AUTH),
        ("user_profiles/settings/spec.md", AUTH),
        ("auth/spec.md", AUTH),
        ("zebra/spec.md", AUTH),
    ]))
    .document;
    let groups: Vec<(&str, &str, usize)> = document.products[0]
        .feature_groups
        .iter()
        .map(|group| {
            (
                group.name.as_str(),
                group.slug.as_str(),
                group.features.len(),
            )
        })
        .collect();
    assert_eq!(
        groups,
        [
            ("Capabilities", "capabilities", 2),
            ("User profiles", "user-profiles", 2)
        ]
    );
}

#[test]
fn a_features_slug_derives_from_the_segments_after_the_first() {
    let conversion = converted(&project(&[
        ("billing/invoices/line-items/spec.md", AUTH),
        ("search/spec.md", AUTH),
    ]));
    let slugs: Vec<&str> = features(&conversion)
        .iter()
        .map(|feature| feature.slug.as_str())
        .collect();
    assert_eq!(slugs, ["invoices-line-items", "search"]);
}

#[test]
fn a_feature_is_named_by_its_title_or_by_its_last_segment_made_readable() {
    let titled = "# Payment Gateway Specification\n\n## Purpose\nPays.\n";
    let untitled = "## Purpose\nItems.\n";
    let bare = "# line_items\n\n## Purpose\nItems.\n";
    let conversion = converted(&project(&[
        ("pay/spec.md", titled),
        ("billing/invoice-items/spec.md", untitled),
        ("line_items/spec.md", bare),
    ]));
    let names: Vec<&str> = features(&conversion)
        .iter()
        .map(|feature| feature.name.as_str())
        .collect();
    assert_eq!(names, ["Invoice items", "Line items", "Payment Gateway"]);
}

#[test]
fn the_purpose_is_the_body_verbatim_and_the_archive_placeholder_leaves_none() {
    let real = "## Purpose\nFirst line.\n\n  Indented second.\n\n## Requirements\n";
    let placeholder =
        "## Purpose\nTBD - created by archiving change add-auth. Update Purpose after archive.\n";
    let conversion = converted(&project(&[("a/spec.md", real), ("b/spec.md", placeholder)]));
    let bodies: Vec<Option<&str>> = features(&conversion)
        .iter()
        .map(|feature| feature.details.body.as_deref())
        .collect();
    assert_eq!(bodies, [Some("First line.\n\n  Indented second."), None]);
    let purposes: Vec<Option<&ExtValue>> = features(&conversion)
        .iter()
        .map(|feature| openspec(&feature.details.ext, "purpose"))
        .collect();
    let kept = "TBD - created by archiving change add-auth. Update Purpose after archive.";
    assert_eq!(purposes, [None, Some(&ExtValue::Text(kept.to_owned()))]);
}

#[test]
fn each_feature_keeps_its_capability_id_and_each_requirement_its_name_in_ext() {
    let conversion = converted(&project(&[
        ("billing/invoices/spec.md", AUTH),
        ("auth/spec.md", AUTH),
    ]));
    let text = |value: Option<&ExtValue>| value.and_then(ExtValue::text).map(str::to_owned);
    let mut capabilities: Vec<Option<String>> = features(&conversion)
        .iter()
        .map(|feature| text(openspec(&feature.details.ext, "capability")))
        .collect();
    capabilities.sort();
    assert_eq!(
        capabilities,
        [Some("auth".to_owned()), Some("billing/invoices".to_owned())]
    );
    let names: Vec<Option<String>> = requirements(&conversion)
        .iter()
        .map(|requirement| text(openspec(&requirement.details.ext, "name")))
        .collect();
    assert_eq!(
        names,
        [Some("Sign in".to_owned()), Some("Sign out".to_owned())]
    );
}

#[test]
fn other_level_two_sections_become_sections_nested_by_heading_level() {
    let text = "# Notes Specification

A preface.

## Purpose
Notes.

## Design Notes
Opening text.

### Storage
On disk.

#### Format
YAML.

### Cache
In memory.

## Requirements
";
    let conversion = converted(&one(text));
    let sections = &features(&conversion)[0].details.sections;
    assert_eq!(names(sections), ["Introduction", "Design Notes"]);
    assert_eq!(sections[0].body, "A preface.");
    let design = &sections[1];
    assert_eq!(design.body, "Opening text.");
    assert_eq!(names(&design.sections), ["Storage", "Cache"]);
    assert_eq!(design.sections[0].body, "On disk.");
    assert_eq!(names(&design.sections[0].sections), ["Format"]);
    assert_eq!(design.sections[0].sections[0].body, "YAML.");
}

#[test]
fn text_of_requirements_outside_every_requirement_becomes_sections() {
    let text = "## Requirements
Read these in order.

### Requirement: Only
The system SHALL be the only one.

### Glossary
Terms used above.

#### Session
A signed-in period.
";
    let conversion = converted(&one(text));
    let sections = &features(&conversion)[0].details.sections;
    assert_eq!(names(sections), ["Requirements", "Glossary"]);
    assert_eq!(sections[0].body, "Read these in order.");
    assert_eq!(sections[1].body, "Terms used above.");
    assert_eq!(names(&sections[1].sections), ["Session"]);
    assert_eq!(sections[1].sections[0].body, "A signed-in period.");
    assert_eq!(requirements(&conversion).len(), 1);
}

#[test]
fn requirements_and_their_scenarios_keep_source_order() {
    let conversion = converted(&one(AUTH));
    let requirements = requirements(&conversion);
    let statements: Vec<String> = requirements
        .iter()
        .map(|requirement| requirement.statement.render())
        .collect();
    assert_eq!(
        statements,
        [
            "The system shall let a user sign in with a password.",
            "When a user signs out, the system shall end the session."
        ]
    );
    let criteria: Vec<&str> = requirements[0]
        .criteria
        .iter()
        .map(|criterion| criterion.name.as_str())
        .collect();
    assert_eq!(criteria, ["Valid credentials", "Wrong password"]);
    assert_eq!(
        requirements[0].criteria[1].details.body.as_deref(),
        Some(
            "- **WHEN** a user submits a wrong password\n- **THEN** the system MUST refuse the sign-in"
        )
    );
}

#[test]
fn a_child_of_a_requirement_that_is_not_a_scenario_becomes_its_section() {
    let text = "## Requirements

### Requirement: Reasoned
The system SHALL keep its reasons.

#### Scenario: Kept
- **THEN** kept

#### Notes
Written for reviewers.
";
    let conversion = converted(&one(text));
    let requirement = &requirements(&conversion)[0];
    assert_eq!(requirement.criteria.len(), 1);
    assert_eq!(names(&requirement.details.sections), ["Notes"]);
    assert_eq!(
        requirement.details.sections[0].body,
        "Written for reviewers."
    );
}

#[test]
fn a_purpose_opening_with_a_tbd_or_todo_marker_is_a_placeholder_kept_in_ext() {
    for placeholder in [
        "TBD - created by syncing change add-auth. Update Purpose after sync.",
        "TODO: say what this is for",
        "tbd.",
    ] {
        let conversion = converted(&one(&format!("## Purpose\n{placeholder}\n")));
        let feature = features(&conversion)[0];
        assert_eq!(feature.details.body, None, "{placeholder}");
        let kept = openspec(&feature.details.ext, "purpose");
        assert_eq!(kept, Some(&ExtValue::Text(placeholder.to_owned())));
    }
    let written = "TBDs are tracked in the backlog of the team.";
    let conversion = converted(&one(&format!("## Purpose\n{written}\n")));
    assert_eq!(
        features(&conversion)[0].details.body.as_deref(),
        Some(written)
    );
}

#[test]
fn the_introductions_headings_become_sections_nested_by_level() {
    let text = "# T Specification

Read this first.

### Requirement: Early
The system SHALL start.

#### Scenario: Started
- **THEN** it runs

## Purpose
The purpose.
";
    let conversion = converted(&one(text));
    let sections = &features(&conversion)[0].details.sections;
    assert_eq!(names(sections), ["Introduction"]);
    assert_eq!(sections[0].body, "Read this first.");
    assert_eq!(names(&sections[0].sections), ["Requirement: Early"]);
    let early = &sections[0].sections[0];
    assert_eq!(early.body, "The system SHALL start.");
    assert_eq!(names(&early.sections), ["Scenario: Started"]);
    assert_eq!(early.sections[0].body, "- **THEN** it runs");
}

#[test]
fn a_requirement_childs_own_headings_become_its_sections_nested_by_level() {
    let text = "## Requirements

### Requirement: Reasoned
The system SHALL keep its reasons.

#### Notes
Written for reviewers.

##### Detail
More.
";
    let conversion = converted(&one(text));
    let notes = &requirements(&conversion)[0].details.sections[0];
    assert_eq!(notes.body, "Written for reviewers.");
    assert_eq!(names(&notes.sections), ["Detail"]);
    assert_eq!(notes.sections[0].body, "More.");
}

#[test]
fn a_title_after_text_nests_in_the_introduction_as_its_section() {
    let text = "Lead text.\n\n# T Specification\n\nAfter the title.\n\n## Purpose\nThe purpose.\n";
    let conversion = converted(&one(text));
    let feature = features(&conversion)[0];
    assert_eq!(feature.name, "T");
    let introduction = &feature.details.sections[0];
    assert_eq!(introduction.body, "Lead text.");
    assert_eq!(names(&introduction.sections), ["T Specification"]);
    assert_eq!(introduction.sections[0].body, "After the title.");
}

//! Reading an `OpenSpec` project: which `spec.md` files are capabilities, their order, the refusal
//! of a project without one, and how a main spec's lines, fences, front matter and headings are
//! read.

use crate::common::openspec::{AUTH, converted, features, one, project, requirements, try_convert};
use loftspec::convert::openspec::{ConvertError, Project};

fn capability_ids(project: &Project) -> Vec<String> {
    converted(project)
        .sidecar
        .nodes
        .iter()
        .filter(|node| node.kind == "feature")
        .map(|node| node.capability.clone().unwrap())
        .collect()
}

#[test]
fn a_spec_at_any_depth_is_a_capability_named_by_its_directory_path() {
    let ids = capability_ids(&project(&[
        ("auth/spec.md", AUTH),
        ("billing/invoices/items/spec.md", AUTH),
    ]));
    assert_eq!(ids, ["auth", "billing/invoices/items"]);
}

#[test]
fn a_spec_below_a_dot_directory_or_directly_in_specs_is_not_read() {
    let ids = capability_ids(&project(&[
        (".drafts/spec.md", AUTH),
        ("auth/.old/spec.md", AUTH),
        ("spec.md", AUTH),
        ("auth/spec.md", AUTH),
    ]));
    assert_eq!(ids, ["auth"]);
}

#[test]
fn capabilities_are_read_in_the_byte_order_of_their_ids() {
    let ids = capability_ids(&project(&[
        ("search/spec.md", AUTH),
        ("auth/spec.md", AUTH),
        ("Zone/spec.md", AUTH),
        ("auth-admin/spec.md", AUTH),
    ]));
    assert_eq!(ids, ["Zone", "auth", "auth-admin", "search"]);
}

#[test]
fn a_project_without_a_main_spec_is_refused_naming_its_active_changes() {
    let changes_only = Project {
        active_changes: vec!["add-login".to_owned(), "fix-search".to_owned()],
        ..Project::default()
    };
    let error = try_convert(&changes_only).unwrap_err();
    assert_eq!(
        error,
        ConvertError::NoMainSpec(vec!["add-login".to_owned(), "fix-search".to_owned()])
    );
    let message = error.to_string();
    assert!(message.contains("add-login, fix-search"), "{message}");
    assert!(try_convert(&project(&[("notes/readme.md", AUTH)])).is_err());
}

#[test]
fn a_byte_order_mark_and_crlf_line_endings_convert_as_without_them() {
    let plain = converted(&one(AUTH)).document;
    let windows = format!("\u{feff}{}", AUTH.replace('\n', "\r\n"));
    assert_eq!(converted(&one(&windows)).document, plain);
}

const FENCED: &str = "# Fences

## Requirements

### Requirement: Shows an example
The system SHALL show an example.

```markdown
### Requirement: Not a requirement
#### Scenario: Not a scenario
```

~~~~
### Requirement: Not one either
~~~~

#### Scenario: Example shown
- **THEN** it is shown
";

#[test]
fn no_heading_inside_fenced_code_is_read_and_the_code_is_kept_verbatim() {
    let conversion = converted(&one(FENCED));
    let [requirement] = requirements(&conversion) else {
        panic!("one requirement");
    };
    assert_eq!(requirement.criteria.len(), 1);
    let body = requirement.details.body.as_deref().unwrap();
    assert!(
        body.starts_with("```markdown\n### Requirement: Not a requirement\n"),
        "{body}"
    );
    assert!(
        body.ends_with("~~~~\n### Requirement: Not one either\n~~~~"),
        "{body}"
    );
}

#[test]
fn a_fence_left_open_runs_to_the_end_of_the_file() {
    let text = "## Requirements

### Requirement: First
The system SHALL do one thing.

```
### Requirement: Hidden
";
    let conversion = converted(&one(text));
    assert_eq!(requirements(&conversion).len(), 1);
}

#[test]
fn front_matter_is_skipped_and_an_unclosed_opening_line_is_content() {
    let closed = format!("---\nstatus: done\n---\n{AUTH}");
    let conversion = converted(&one(&closed));
    let feature = features(&conversion)[0];
    assert_eq!(feature.name, "Auth");
    assert_eq!(feature.details.sections, []);
    let unclosed = format!("---\nstatus: done\n{AUTH}");
    let conversion = converted(&one(&unclosed));
    let introduction = &features(&conversion)[0].details.sections[0];
    assert_eq!(introduction.name, "Introduction");
    assert!(
        introduction.body.starts_with("---\nstatus: done"),
        "{}",
        introduction.body
    );
}

#[test]
fn requirement_headings_are_matched_without_regard_to_letter_case() {
    let text = "## PURPOSE
Kept.

## requirements

### REQUIREMENT: Loud ##
The system SHALL be loud.
";
    let conversion = converted(&one(text));
    assert_eq!(
        features(&conversion)[0].details.body.as_deref(),
        Some("Kept.")
    );
    assert_eq!(requirements(&conversion).len(), 1);
    let names: Vec<_> = conversion
        .sidecar
        .nodes
        .iter()
        .filter(|node| node.kind == "requirement")
        .map(|node| node.name.as_deref().unwrap())
        .collect();
    assert_eq!(names, ["Loud"]);
}

#[test]
fn a_requirement_heading_outside_requirements_is_not_a_requirement() {
    let text = "## Requirements

### Requirement: Real
The system SHALL be real.

## Notes

### Requirement: Elsewhere
Only a note.
";
    let conversion = converted(&one(text));
    let [requirement] = requirements(&conversion) else {
        panic!("one requirement");
    };
    assert_eq!(
        requirement.details.body, None,
        "the block ends at the next level-2 heading"
    );
    let notes = &features(&conversion)[0].details.sections[0];
    assert_eq!(notes.name, "Notes");
    assert_eq!(notes.sections[0].name, "Requirement: Elsewhere");
}

#[test]
fn each_level_four_heading_is_a_child_and_a_scenario_heading_names_a_scenario() {
    let text = "## Requirements

### Requirement: Children
The system SHALL have children.

#### scenario:   Spaced name
- **THEN** it reads

##### Detail
Still the scenario's text.

#### Rationale
Why it is so.

# Appendix
Outside.
";
    let conversion = converted(&one(text));
    let requirement = &requirements(&conversion)[0];
    let [criterion] = requirement.criteria.as_slice() else {
        panic!("one criterion");
    };
    assert_eq!(criterion.name, "Spaced name");
    assert_eq!(
        criterion.details.body.as_deref(),
        Some("- **THEN** it reads\n\n##### Detail\nStill the scenario's text.")
    );
    let [rationale] = requirement.details.sections.as_slice() else {
        panic!("one Section");
    };
    assert_eq!(rationale.name, "Rationale");
    assert_eq!(rationale.body, "Why it is so.");
}

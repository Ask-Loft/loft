//! The `OpenSpec` projection: where each Feature is written, what its `spec.md` holds, how each
//! Requirement, statement, scenario and Section is written, and the render map.

use loftspec::document::Document;
use loftspec::ext::ExtValue;
use loftspec::names::Namespace;
use loftspec::render::openspec::{OpenspecProjection, RENDER_MAP, RenderError, openspec};
use loftspec::statement::Statement;
use serde_json::Value;

use crate::common::openspec::{AUTH, converted, one, project as loftspec_project};
use crate::common::{criterion, key, section, small_document};

fn rendered(document: &Document) -> OpenspecProjection {
    openspec(document).unwrap()
}

/// The one file the projection of `document` writes.
fn only(document: &Document) -> String {
    let files = rendered(document).files;
    assert_eq!(files.len(), 1, "{:?}", files.keys());
    files.into_values().next().unwrap()
}

/// The document an `OpenSpec` spec converts to.
fn from_openspec(text: &str) -> Document {
    converted(&one(text)).document
}

/// Sets `field` of a node's `ext.openspec` to `value`.
fn set(ext: &mut loftspec::ext::Ext, field: &str, value: &str) {
    let namespace = Namespace::parse("openspec").unwrap();
    let entry = ext
        .entry(namespace)
        .or_insert_with(|| ExtValue::Map(std::collections::BTreeMap::new()));
    let ExtValue::Map(map) = entry else {
        panic!("a mapping")
    };
    map.insert(field.to_owned(), ExtValue::Text(value.to_owned()));
}

fn first_feature(document: &mut Document) -> &mut loftspec::tree::Feature {
    &mut document.products[0].feature_groups[0].features[0]
}

#[test]
fn one_product_renders_under_specs_and_each_of_several_under_its_slug() {
    let paths = |document: &Document| rendered(document).files.into_keys().collect::<Vec<_>>();
    assert_eq!(paths(&small_document()), ["specs/identity/keys/spec.md"]);
    assert_eq!(rendered(&small_document()).roots, ["specs"]);
    let mut document = small_document();
    let mut second = document.products[0].clone();
    second.id = Some(loftspec::ids::ProductId::new(key(90)));
    second.slug = loftspec::slug::Slug::derive("Second", "product");
    let feature = &mut second.feature_groups[0].features[0];
    feature.id = Some(loftspec::ids::FeatureId::new(key(91)));
    feature.requirements.clear();
    second.feature_groups[0].id = Some(loftspec::ids::FeatureGroupId::new(key(92)));
    second.feature_groups[0].constraints.clear();
    second.feature_groups[0].features[0]
        .quality_attributes
        .clear();
    second.constraints.clear();
    second.details.sections.clear();
    document.products.push(second);
    assert_eq!(
        paths(&document),
        [
            "example-product/specs/identity/keys/spec.md",
            "second/specs/identity/keys/spec.md"
        ]
    );
    assert_eq!(
        rendered(&document).roots,
        ["example-product/specs", "second/specs"]
    );
}

#[test]
fn two_renders_of_one_document_are_byte_identical() {
    let document = from_openspec(AUTH);
    assert_eq!(rendered(&document), rendered(&document));
}

#[test]
fn a_feature_is_written_at_its_ext_capability_or_at_its_group_and_slug() {
    let mut document = small_document();
    set(
        &mut first_feature(&mut document).details.ext,
        "capability",
        "auth/login",
    );
    assert_eq!(
        rendered(&document).files.into_keys().collect::<Vec<_>>(),
        ["specs/auth/login/spec.md"]
    );
    for unsafe_path in [
        "",
        "/etc",
        "a//b",
        "../out",
        "a/./b",
        "a\\b",
        ".hidden/auth",
        "a/.b",
    ] {
        let mut document = small_document();
        set(
            &mut first_feature(&mut document).details.ext,
            "capability",
            unsafe_path,
        );
        assert_eq!(
            rendered(&document).files.into_keys().collect::<Vec<_>>(),
            ["specs/identity/keys/spec.md"],
            "{unsafe_path:?}"
        );
    }
}

#[test]
fn a_spec_opens_with_its_name_then_purpose_then_requirements_then_its_sections() {
    let mut document = small_document();
    let feature = first_feature(&mut document);
    let mut notes = section(60, "Notes", "Why keys.");
    notes.sections.push(section(61, "Detail", "More."));
    feature.details.sections = vec![notes];
    let text = only(&document);
    assert!(
        text.starts_with("# Keys Specification\n\n## Purpose\n\nKeys address nodes.\n\n## Requirements\n\n### Requirement: RQ-2\n"),
        "{text}"
    );
    assert!(
        text.ends_with("\n## Notes\n\nWhy keys.\n\n### Detail\nMore.\n"),
        "{text}"
    );
}

#[test]
fn the_purpose_is_the_body_else_the_ext_purpose_else_empty() {
    let mut document = small_document();
    let feature = first_feature(&mut document);
    feature.details.body = None;
    let text = only(&document);
    assert!(text.contains("## Purpose\n\n## Requirements"), "{text}");
    set(
        &mut first_feature(&mut document).details.ext,
        "purpose",
        "TBD - kept.",
    );
    assert!(only(&document).contains("## Purpose\n\nTBD - kept.\n\n## Requirements"));
    first_feature(&mut document).details.body = Some("The body.".to_owned());
    assert!(only(&document).contains("## Purpose\n\nThe body.\n\n## Requirements"));
}

#[test]
fn a_requirement_is_named_by_its_ext_name_else_its_label_else_its_key() {
    let mut document = small_document();
    let text = only(&document);
    assert!(text.contains("### Requirement: RQ-2\n"), "{text}");
    let requirement = &mut first_feature(&mut document).requirements[0];
    set(&mut requirement.details.ext, "name", "Write keys");
    assert!(only(&document).contains("### Requirement: Write keys\n"));
    let mut unlabelled = small_document();
    unlabelled.bindings = loftspec::bindings::Bindings::default();
    let heading = format!("### Requirement: {}\n", key(4));
    assert!(only(&unlabelled).contains(&heading));
}

#[test]
fn requirements_and_their_scenarios_follow_the_documents_order() {
    let text = only(&from_openspec(AUTH));
    let order = [
        "### Requirement: Sign in",
        "#### Scenario: Valid credentials",
        "#### Scenario: Wrong password",
        "### Requirement: Sign out",
    ];
    let at: Vec<usize> = order.iter().map(|line| text.find(line).unwrap()).collect();
    assert!(at.is_sorted(), "{text}");
}

#[test]
fn a_statement_in_slots_is_written_with_its_ext_modal_or_shall_and_text_as_stored() {
    let mut document = small_document();
    assert!(only(&document).contains("\nThe writer SHALL write a key on every node.\n"));
    let requirement = &mut first_feature(&mut document).requirements[0];
    set(&mut requirement.details.ext, "modal", "MUST");
    assert!(only(&document).contains("\nThe writer MUST write a key on every node.\n"));
    let requirement = &mut first_feature(&mut document).requirements[0];
    set(&mut requirement.details.ext, "modal", "shall");
    assert!(only(&document).contains("\nThe writer shall write a key on every node.\n"));
    let requirement = &mut first_feature(&mut document).requirements[0];
    set(&mut requirement.details.ext, "modal", "may");
    assert!(only(&document).contains("\nThe writer SHALL write a key on every node.\n"));
    let requirement = &mut first_feature(&mut document).requirements[0];
    requirement.statement = Statement::Text("Keys shall be minted, ok".to_owned());
    assert!(only(&document).contains("\nKeys shall be minted, ok\n"));
}

#[test]
fn metadata_lines_then_the_body_follow_the_statement() {
    let spec = "## Requirements\n\n### Requirement: R\n**ID**: R-1\nThe system SHALL r.\n**Owner**: ops\n\nMore detail.\n\n#### Scenario: S\n- **THEN** r\n";
    let text = only(&from_openspec(spec));
    assert!(
        text.contains("### Requirement: R\nThe system SHALL r.\n**ID**: R-1\n**Owner**: ops\n\nMore detail.\n\n#### Scenario: S\n- **THEN** r\n"),
        "{text}"
    );
}

#[test]
fn a_criterion_is_a_scenario_with_its_body_or_a_then_line_of_its_name() {
    let text = only(&small_document());
    assert!(
        text.contains("\n#### Scenario: Every node record carries a `key` field.\n- **THEN** Every node record carries a `key` field.\n"),
        "{text}"
    );
    let text = only(&from_openspec(AUTH));
    assert!(text.contains("#### Scenario: Wrong password\n- **WHEN** a user submits a wrong password\n- **THEN** the system MUST refuse the sign-in\n"), "{text}");
}

#[test]
fn a_requirements_sections_follow_its_scenarios_one_level_deeper_each() {
    let mut document = small_document();
    let requirement = &mut first_feature(&mut document).requirements[0];
    let mut outer = section(62, "Why", "Because.");
    outer.sections.push(section(63, "Deeper", "Still."));
    requirement.details.sections = vec![outer];
    requirement.criteria.push(criterion(key(64), "Second"));
    let text = only(&document);
    assert!(
        text.contains("#### Scenario: Second\n- **THEN** Second\n\n#### Why\nBecause.\n\n##### Deeper\nStill.\n"),
        "{text}"
    );
}

/// The render map's entries.
fn entries(projection: &OpenspecProjection) -> Vec<Value> {
    let map: Value = serde_json::from_str(&projection.map).unwrap();
    assert_eq!(map["schema"], "loft-render-map/1");
    map["entries"].as_array().unwrap().clone()
}

#[test]
fn the_render_map_records_each_requirement_and_scenario_with_its_file_line_key_and_label() {
    assert_eq!(RENDER_MAP, "loft-render-map.json");
    let projection = rendered(&small_document());
    let text = &projection.files["specs/identity/keys/spec.md"];
    let line = |prefix: &str| {
        text.lines()
            .position(|line| line.starts_with(prefix))
            .unwrap()
            + 1
    };
    assert_eq!(
        entries(&projection),
        [
            serde_json::json!({
                "file": "specs/identity/keys/spec.md",
                "line": line("### Requirement:"),
                "kind": "requirement",
                "key": key(4).to_string(),
                "label": "RQ-2",
            }),
            serde_json::json!({
                "file": "specs/identity/keys/spec.md",
                "line": line("#### Scenario:"),
                "kind": "scenario",
                "key": key(5).to_string(),
            }),
        ]
    );
}

/// A spec that exercises each part the projection writes back: an archive placeholder Purpose,
/// metadata lines, `MUST` and `SHALL`, a body, scenarios, a child that is not a scenario, a text
/// statement and a level-2 section after the requirements.
const ROUND_TRIP: &str = "# auth Specification

## Purpose

TBD - created by archiving change add-auth. Update Purpose after archive.

## Requirements

### Requirement: Sign in
**ID**: AUTH-1
The system MUST let a user sign in with a password.

A locked account cannot sign in.

#### Scenario: Valid
- **WHEN** a user gives the right password
- **THEN** the user is signed in

#### Notes
Not a scenario.

### Requirement: Audit
The system keeps a log, as the auditors ask.

#### Scenario: Logged
- **THEN** each sign-in is logged

## Glossary

Session: the time a user stays signed in.
";

/// A spec with headings above its first level-2 heading, a requirement's child with a heading
/// of its own, and a `MUST` with a lower-case `shall` after it.
const NESTED: &str = "# notes Specification

Read this first.

### Requirement: Early
The system SHALL start.

## Purpose

Notes keep what the team decided and why it decided it, for later readers.

## Requirements

### Requirement: Keep
The system MUST log what users shall see.

#### Scenario: Logged
- **THEN** it is logged

#### Notes
Why.

##### Detail
More.
";

#[test]
fn an_imported_project_renders_to_specs_that_import_to_the_same_document() {
    let first = converted(&loftspec_project(&[
        ("auth/spec.md", ROUND_TRIP),
        ("billing/invoices/spec.md", AUTH),
        ("notes/spec.md", NESTED),
        (
            "lead/spec.md",
            "Lead text.\n\n# lead Specification\n\n## Purpose\nThe purpose.\n",
        ),
    ]));
    let projection = rendered(&first.document);
    let specs: Vec<(String, String)> = projection
        .files
        .iter()
        .map(|(path, text)| {
            let path = path.strip_prefix("specs/").unwrap().to_owned();
            (path, text.clone())
        })
        .collect();
    let paths: Vec<(&str, &str)> = specs
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let second = converted(&loftspec_project(&paths));
    assert_eq!(second.document, first.document);
}

#[test]
fn two_features_written_to_one_path_are_refused_naming_the_path_and_both() {
    for capability in ["auth", "Auth"] {
        let mut document = converted(&loftspec_project(&[
            ("auth/spec.md", AUTH),
            ("billing/spec.md", AUTH),
        ]))
        .document;
        let billing = &mut document.products[0].feature_groups[0].features[1];
        set(&mut billing.details.ext, "capability", capability);
        let Err(RenderError::Collision { path, features }) = openspec(&document) else {
            panic!("{capability}: a collision")
        };
        assert_eq!(path.to_lowercase(), "specs/auth/spec.md", "{capability}");
        assert_ne!(features[0], features[1]);
        let error = RenderError::Collision { path, features }.to_string();
        assert!(
            error.to_lowercase().contains("specs/auth/spec.md"),
            "{error}"
        );
    }
}

/// A Section nested `depth` levels below `section`, each named by its depth.
fn nested(depth: u32) -> loftspec::tree::Section {
    let mut innermost = section(70 + depth, &format!("Depth {depth}"), "Deep.");
    for level in (1..depth).rev() {
        let mut outer = section(70 + level, &format!("Depth {level}"), "Deep.");
        outer.sections.push(innermost);
        innermost = outer;
    }
    innermost
}

#[test]
fn a_section_that_would_be_deeper_than_level_six_is_written_at_level_six() {
    let mut document = small_document();
    first_feature(&mut document).details.sections = vec![nested(6)];
    let text = only(&document);
    assert!(
        text.contains("\n###### Depth 5\nDeep.\n\n###### Depth 6\nDeep.\n"),
        "{text}"
    );
    let mut document = small_document();
    let feature = first_feature(&mut document);
    feature.details.sections.clear();
    feature.requirements[0].details.sections = vec![nested(4)];
    let text = only(&document);
    assert!(
        text.contains("\n###### Depth 3\nDeep.\n\n###### Depth 4\nDeep.\n"),
        "{text}"
    );
}

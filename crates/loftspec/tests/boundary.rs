//! A Component's boundary (F-50, ADR-15): the field in the text form and the content view,
//! how a subtree takes it on, the validator's rule, the obligation rules and the rendering.

mod common;

use common::text::{edited, files, text};
use common::{key, small_document};
use loftspec::attributes::Boundary;
use loftspec::boundary::Boundaries;
use loftspec::canonical::canonical_bytes;
use loftspec::codec::{self, Problem, Sites};
use loftspec::document::Document;
use loftspec::hash::ContentHash;
use loftspec::identity::Key;
use loftspec::ids::{ComponentId, ConstraintId};
use loftspec::lint::{LintConfig, RuleId, lint};
use loftspec::render::render;
use loftspec::tree::{Constraint, Details};
use loftspec::validate::{ValidationError, validate};

const COMPONENT: &str = "components/document-model.yaml";

fn external_document() -> Document {
    let mut document = small_document();
    document.components[0].boundary = Some(Boundary::External);
    document
}

#[test]
fn boundary_is_written_after_the_body_and_tags_and_before_scoped_to() {
    let written = codec::write(&external_document());
    let component = String::from_utf8(written[&common::text::path(COMPONENT)].clone()).unwrap();
    let boundary = component.find("\nboundary: external\n").expect("written");
    assert!(boundary > component.find("\nbody: ").unwrap());
    assert!(boundary < component.find("\nscopedTo:").unwrap());
}

#[test]
fn boundary_reads_back_as_written() {
    let document = external_document();
    let read = codec::read(&codec::write(&document)).unwrap();
    assert_eq!(read.components[0].boundary, Some(Boundary::External));
    assert_eq!(codec::write(&read), codec::write(&document));
}

#[test]
fn a_component_without_boundary_reads_as_none() {
    assert_eq!(codec::read(&files()).unwrap().components[0].boundary, None);
    assert!(!text(&files(), COMPONENT).contains("boundary"));
}

#[test]
fn another_boundary_value_is_an_error_at_its_position() {
    let files = edited(COMPONENT, "scopedTo:", "boundary: outside\nscopedTo:");
    let errors = codec::read(&files).expect_err("refused");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        matches!(&errors[0].problem, Problem::Invalid { value, .. } if value == "outside"),
        "{errors:?}"
    );
    assert_eq!(
        errors[0].sites[0].to_string(),
        "spec/components/document-model.yaml:7:11"
    );
}

#[test]
fn the_boundary_enters_the_content_view_as_written() {
    let mut component = small_document().components[0].component.clone();
    let view = |component: &loftspec::tree::Component| {
        String::from_utf8(canonical_bytes(&component.content_view()[9])).unwrap()
    };
    assert_eq!(view(&component), "null");
    component.boundary = Some(Boundary::External);
    assert_eq!(view(&component), "\"external\"");
    component.boundary = Some(Boundary::Private);
    assert_eq!(view(&component), "\"private\"");
}

#[test]
fn an_explicit_private_and_an_absent_boundary_give_different_views() {
    let mut component = small_document().components[0].component.clone();
    let absent = component.content_hash();
    component.boundary = Some(Boundary::Private);
    assert_ne!(component.content_hash(), absent);
}

#[test]
fn a_node_that_is_not_a_component_has_a_null_boundary_slot() {
    let feature = &small_document().products[0].feature_groups[0].features[0];
    assert_eq!(feature.content_view()[9], serde_json::Value::Null);
}

// Inheritance and the validator.

fn nested_document(parent: Option<Boundary>, child: Option<Boundary>) -> Document {
    let mut document = small_document();
    let mut part = common::component(key(40), "Part");
    part.boundary = child;
    document.components[0].boundary = parent;
    document.components[0].components.push(part);
    document
}

#[test]
fn a_nested_component_without_boundary_takes_its_parents() {
    let boundaries = Boundaries::of(&nested_document(Some(Boundary::External), None));
    assert!(boundaries.is_external(ComponentId::new(key(40))));
}

#[test]
fn a_root_component_without_boundary_is_private() {
    let boundaries = Boundaries::of(&nested_document(None, None));
    assert!(!boundaries.is_external(ComponentId::new(key(6))));
    assert!(!boundaries.is_external(ComponentId::new(key(40))));
}

#[test]
fn a_written_boundary_is_the_components_own() {
    let boundaries = Boundaries::of(&nested_document(None, Some(Boundary::External)));
    assert!(!boundaries.is_external(ComponentId::new(key(6))));
    assert!(boundaries.is_external(ComponentId::new(key(40))));
}

#[test]
fn private_beneath_external_is_an_error_naming_both() {
    let document = nested_document(Some(Boundary::External), Some(Boundary::Private));
    let errors = validate(&document);
    assert_eq!(
        errors,
        [ValidationError::PrivateUnderExternal {
            component: ComponentId::new(key(40)),
            ancestor: ComponentId::new(key(6)),
        }]
    );
    assert_eq!(errors[0].keys(), [key(40), key(6)]);
}

#[test]
fn external_beneath_external_raises_no_error() {
    let document = nested_document(Some(Boundary::External), Some(Boundary::External));
    assert_eq!(validate(&document), Vec::new());
}

// The obligation rules.

fn findings(document: &Document, rule: RuleId) -> Vec<Vec<Key>> {
    lint(document, &Sites::default(), &LintConfig::default())
        .into_iter()
        .filter(|finding| finding.rule == rule)
        .map(|finding| finding.subjects.iter().map(|subject| subject.key).collect())
        .collect()
}

#[test]
fn an_external_leaf_component_yields_no_sh_002() {
    let mut document = small_document();
    document
        .components
        .push(common::root(common::component(key(41), "Voyage API")));
    assert_eq!(findings(&document, RuleId::Sh002), [vec![key(41)]]);
    document.components[1].boundary = Some(Boundary::External);
    assert_eq!(findings(&document, RuleId::Sh002), Vec::<Vec<Key>>::new());
}

#[test]
fn an_external_component_yields_no_sh_008() {
    let mut document = small_document();
    document
        .components
        .push(common::root(common::component(key(41), "Voyage API")));
    assert_eq!(findings(&document, RuleId::Sh008), [vec![key(41)]]);
    document.components[1].boundary = Some(Boundary::External);
    assert_eq!(findings(&document, RuleId::Sh008), Vec::<Vec<Key>>::new());
}

#[test]
fn an_external_childs_constraint_does_not_clear_sh_008_above_it() {
    let mut document = small_document();
    let mut service = common::component(key(42), "Turnstile");
    service.boundary = Some(Boundary::External);
    service.constraints.push(Constraint {
        id: Some(ConstraintId::new(key(43))),
        name: "Tokens shall be verified server-side.".to_owned(),
        details: Details::default(),
        binding: None,
        conservation_kind: None,
        verification_fate: None,
    });
    let mut worker = common::component(key(41), "Website worker");
    worker.components.push(service);
    document.components.push(common::root(worker));
    assert_eq!(findings(&document, RuleId::Sh008), [vec![key(41)]]);
}

// The rendering.

#[test]
fn a_component_with_a_boundary_renders_a_line_naming_it() {
    let markdown = render(&external_document()).unwrap().markdown;
    assert!(markdown.contains("\nBoundary: `external`\n"), "{markdown}");
    let plain = render(&small_document()).unwrap().markdown;
    assert!(!plain.contains("Boundary:"));
}

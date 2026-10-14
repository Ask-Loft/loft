//! The fields of each node record: written in the Format's order, and refused by the reader
//! when a required one is missing. Each expected order is the published list, written out here
//! rather than taken from the record schema the writer follows.

use crate::common::fields::{assert_required, fields};
use crate::common::full::{full_files, full_nested_document};
use crate::common::small_document;
use crate::common::text::{FEATURE, PRODUCT, path, text};
use loftspec::attributes::QualityBand;
use loftspec::codec::{self, Problem, Site};
use loftspec::statement::{EarsForm, Statement};
use loftspec::yaml::{Location, Position};

const COMPONENT: &str = "components/document-model.yaml";
const GROUP: &[&str] = &["featureGroups", "0"];
const REQUIREMENT: &[&str] = &["requirements", "0"];
const CRITERION: &[&str] = &["requirements", "0", "criteria", "0"];
const STATEMENT: &[&str] = &["requirements", "0", "statement"];
const NESTED: &[&str] = &["components", "0"];
/// The Product's own Constraint, which holds every optional field.
const CONSTRAINT: &[&str] = &["constraints", "0"];
const QUALITY: &[&str] = &["qualityAttributes", "0"];
const TARGET: &[&str] = &["qualityAttributes", "0", "qualityTarget"];
const SECTION: &[&str] = &["sections", "0"];

#[test]
fn a_product_records_fields_are_written_in_the_published_order() {
    let published = [
        "key",
        "slug",
        "name",
        "body",
        "tags",
        "mentions",
        "featureGroups",
        "constraints",
        "sections",
    ];
    assert_eq!(fields(&full_files(), PRODUCT, &[]), published);
}

#[test]
fn a_product_record_without_its_key_slug_or_name_is_refused() {
    for field in ["key", "slug", "name"] {
        assert_required(&full_files(), PRODUCT, &[], field);
    }
}

#[test]
fn a_feature_group_records_fields_are_written_in_the_published_order() {
    let published = [
        "key",
        "slug",
        "name",
        "body",
        "tags",
        "mentions",
        "features",
        "constraints",
        "sections",
    ];
    assert_eq!(fields(&full_files(), PRODUCT, GROUP), published);
}

#[test]
fn a_feature_group_record_without_its_key_slug_or_name_is_refused() {
    for field in ["key", "slug", "name"] {
        assert_required(&full_files(), PRODUCT, GROUP, field);
    }
}

#[test]
fn a_feature_records_fields_are_written_in_the_published_order() {
    let published = [
        "key",
        "slug",
        "name",
        "body",
        "tags",
        "realizes",
        "mentions",
        "requirements",
        "qualityAttributes",
        "sections",
    ];
    assert_eq!(fields(&full_files(), FEATURE, &[]), published);
}

#[test]
fn a_feature_record_without_its_key_slug_or_name_is_refused() {
    for field in ["key", "slug", "name"] {
        assert_required(&full_files(), FEATURE, &[], field);
    }
}

#[test]
fn a_requirement_records_fields_are_written_in_the_published_order() {
    let published = [
        "key",
        "statement",
        "body",
        "tags",
        "mentions",
        "criteria",
        "sections",
    ];
    assert_eq!(fields(&full_files(), FEATURE, REQUIREMENT), published);
}

#[test]
fn a_requirement_record_without_its_key_or_statement_is_refused() {
    for field in ["key", "statement"] {
        assert_required(&full_files(), FEATURE, REQUIREMENT, field);
    }
}

#[test]
fn a_criterion_records_fields_are_written_in_the_published_order() {
    let published = ["key", "name", "body", "tags", "mentions", "sections"];
    assert_eq!(fields(&full_files(), FEATURE, CRITERION), published);
}

#[test]
fn a_criterion_record_without_its_key_or_name_is_refused() {
    for field in ["key", "name"] {
        assert_required(&full_files(), FEATURE, CRITERION, field);
    }
}

#[test]
fn a_statement_stored_as_text_has_the_single_field_text() {
    let mut document = small_document();
    let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
    requirement.statement = Statement::Text("The writer writes keys.".to_owned());
    let files = codec::write(&document);
    assert_eq!(fields(&files, FEATURE, STATEMENT), ["text"]);
}

/// The small document's files with its Requirement's statement in the form whose condition
/// slot is `condition`, or in the ubiquitous form for none.
fn with_form(condition: Option<&str>) -> codec::Files {
    let (slot, system, shall) = (
        "a condition".to_owned(),
        "The writer".to_owned(),
        "write a key.".to_owned(),
    );
    let form = match condition {
        None => EarsForm::Ubiquitous { system, shall },
        Some("when") => EarsForm::EventDriven {
            when: slot,
            system,
            shall,
        },
        Some("while") => EarsForm::StateDriven {
            r#while: slot,
            system,
            shall,
        },
        Some("if") => EarsForm::Unwanted {
            r#if: slot,
            system,
            shall,
        },
        Some(_) => EarsForm::Optional {
            r#where: slot,
            system,
            shall,
        },
    };
    let mut document = small_document();
    let requirement = &mut document.products[0].feature_groups[0].features[0].requirements[0];
    requirement.statement = Statement::Form(form);
    codec::write(&document)
}

#[test]
fn a_statement_stored_as_a_form_has_form_then_its_condition_then_system_and_shall() {
    for condition in [None, Some("when"), Some("while"), Some("if"), Some("where")] {
        let published: Vec<&str> = ["form"]
            .into_iter()
            .chain(condition)
            .chain(["system", "shall"])
            .collect();
        let written = fields(&with_form(condition), FEATURE, STATEMENT);
        assert_eq!(written, published, "{condition:?}");
    }
}

#[test]
fn a_component_records_fields_are_written_in_the_published_order_at_the_root_and_beneath_it() {
    let files = codec::write(&full_nested_document());
    let published = [
        "key",
        "name",
        "body",
        "tags",
        "boundary",
        "scopedTo",
        "mentions",
        "components",
        "constraints",
        "qualityAttributes",
        "sections",
    ];
    assert_eq!(fields(&files, COMPONENT, NESTED), published);
    let mut root = published.to_vec();
    root.insert(1, "slug");
    assert_eq!(fields(&files, COMPONENT, &[]), root);
}

#[test]
fn a_component_record_without_its_key_or_name_is_refused_at_the_root_and_beneath_it() {
    for route in [&[][..], NESTED] {
        for field in ["key", "name"] {
            assert_required(&full_files(), COMPONENT, route, field);
        }
    }
}

#[test]
fn a_root_component_record_carries_its_slug_after_its_key_and_requires_it() {
    let written = fields(&full_files(), COMPONENT, &[]);
    assert_eq!(written[..2], ["key", "slug"]);
    assert_required(&full_files(), COMPONENT, &[], "slug");
}

#[test]
fn a_nested_component_record_carrying_a_slug_is_refused() {
    let mut files = full_files();
    let nested = "  - key: 00000000-0000-7000-8000-00000000003d\n";
    let before = text(&files, COMPONENT);
    let line = before[..before.find(nested).unwrap()].lines().count() + 2;
    let after = before.replacen(nested, &format!("{nested}    slug: key-parser\n"), 1);
    files.insert(path(COMPONENT), after.into_bytes());
    let written = fields(&full_files(), COMPONENT, NESTED);
    assert!(!written.iter().any(|field| field == "slug"), "{written:?}");
    let errors = codec::read(&files).expect_err("refused");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].problem, Problem::UnknownField("slug".to_owned()));
    let site = Site {
        path: path(COMPONENT),
        location: Some(Location::At(Position::new(line, 5))),
    };
    assert_eq!(errors[0].sites, [site]);
}

#[test]
fn a_constraint_records_fields_are_written_in_the_published_order() {
    let published = [
        "key",
        "name",
        "body",
        "tags",
        "constraintBinding",
        "conservationKind",
        "verificationFate",
        "mentions",
        "sections",
    ];
    assert_eq!(fields(&full_files(), PRODUCT, CONSTRAINT), published);
}

#[test]
fn a_constraint_record_without_its_key_or_name_is_refused() {
    for field in ["key", "name"] {
        assert_required(&full_files(), PRODUCT, CONSTRAINT, field);
    }
}

#[test]
fn a_constraint_binding_holds_its_category_then_its_required_value_and_requires_both() {
    let binding = [CONSTRAINT, &["constraintBinding"]].concat();
    let written = fields(&full_files(), PRODUCT, &binding);
    assert_eq!(written, ["category", "requiredValue"]);
    for field in ["category", "requiredValue"] {
        assert_required(&full_files(), PRODUCT, &binding, field);
    }
}

#[test]
fn a_quality_attribute_records_fields_are_written_in_the_published_order() {
    let published = [
        "key",
        "name",
        "body",
        "tags",
        "qaKind",
        "qualityTarget",
        "verificationFate",
        "mentions",
        "sections",
    ];
    assert_eq!(fields(&full_files(), FEATURE, QUALITY), published);
}

#[test]
fn a_quality_attribute_record_without_its_key_name_kind_or_target_is_refused() {
    for field in ["key", "name", "qaKind", "qualityTarget"] {
        assert_required(&full_files(), FEATURE, QUALITY, field);
    }
}

#[test]
fn a_quality_target_holds_its_six_fields_in_the_published_order_and_requires_each() {
    let published = [
        "metric",
        "band",
        "unit",
        "aggregation",
        "scenario",
        "environment",
    ];
    assert_eq!(fields(&full_files(), FEATURE, TARGET), published);
    for field in published {
        assert_required(&full_files(), FEATURE, TARGET, field);
    }
}

#[test]
fn a_band_holds_its_kind_and_then_the_bounds_its_kind_takes_in_order() {
    let bound = || "1".to_owned();
    let bands = [
        (
            QualityBand::AtMost { upper: bound() },
            &["kind", "upper"][..],
        ),
        (QualityBand::AtLeast { lower: bound() }, &["kind", "lower"]),
        (QualityBand::Exactly { value: bound() }, &["kind", "value"]),
        (
            QualityBand::Between {
                lower: bound(),
                upper: bound(),
            },
            &["kind", "lower", "upper"],
        ),
    ];
    for (band, published) in bands {
        let mut document = small_document();
        document.components[0].quality_attributes[0].target.band = band;
        let files = codec::write(&document);
        let route = ["qualityAttributes", "0", "qualityTarget", "band"];
        assert_eq!(fields(&files, COMPONENT, &route), published);
    }
}

#[test]
fn a_section_records_fields_are_written_in_the_published_order() {
    let published = ["key", "name", "body", "mentions", "sections"];
    assert_eq!(fields(&full_files(), PRODUCT, SECTION), published);
}

#[test]
fn a_section_record_without_its_key_name_or_body_is_refused() {
    for field in ["key", "name", "body"] {
        assert_required(&full_files(), PRODUCT, SECTION, field);
    }
}

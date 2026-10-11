//! Where the reader says an element is written, every error of every file, and the
//! containment and edge-kind rules that the typed text form refuses as it reads (D-11 F-17,
//! F-18, F-47).

mod common;

use common::key;
use common::text::{FEATURE, PRODUCT, edited, files, path, text};
use loftspec::codec::{self, Files, Problem, ReadError};

const COMPONENT: &str = "components/document-model.yaml";
const DECISION: &str = "decisions/keys-are-uuids.yaml";
const FEATURE_KEY: &str = "00000000-0000-7000-8000-000000000003";
const REQUIREMENT_KEY: &str = "00000000-0000-7000-8000-000000000004";
const COMPONENT_KEY: &str = "00000000-0000-7000-8000-000000000006";
const DECISION_KEY: &str = "00000000-0000-7000-8000-000000000014";

/// The one error a tree gives, which must name the file at `at`.
fn refused_in(files: &Files, at: &str) -> ReadError {
    let errors = codec::read(files).expect_err("refused");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].sites[0].path, path(at), "{errors:?}");
    errors[0].clone()
}

fn model_problem(error: &ReadError) -> &str {
    match &error.problem {
        Problem::Model(problem) => problem,
        other => panic!("{other:?}"),
    }
}

#[test]
fn ac_592_the_site_of_a_key_is_its_file_line_and_column() {
    let (_, sites) = codec::read_sited(&files()).unwrap();
    assert_eq!(
        sites.get(key(3)).unwrap().to_string(),
        "spec/products/example-product/identity/keys.yaml:1:6"
    );
    assert_eq!(
        sites.get(key(5)).unwrap().to_string(),
        "spec/products/example-product/identity/keys.yaml:14:14"
    );
    assert_eq!(
        sites.get(key(30)).unwrap().to_string(),
        "spec/tags.yaml:1:8"
    );
}

#[test]
fn ac_527_an_unresolved_key_in_each_of_two_files_gives_two_errors() {
    let mut files = edited(
        FEATURE,
        &format!("  - {COMPONENT_KEY}"),
        "  - 00000000-0000-7000-8000-000000000077",
    );
    let decision = text(&files, DECISION).replacen(
        &format!("    node: {COMPONENT_KEY}"),
        "    node: 00000000-0000-7000-8000-000000000078",
        1,
    );
    files.insert(path(DECISION), decision.into_bytes());
    let errors = codec::read(&files).expect_err("refused");
    let paths: Vec<String> = errors
        .iter()
        .map(|error| error.sites[0].path.to_string())
        .collect();
    assert_eq!(
        paths,
        [
            "spec/products/example-product/identity/keys.yaml",
            "spec/decisions/keys-are-uuids.yaml",
        ]
    );
}

#[test]
fn ac_317_a_child_kind_outside_the_profile_is_refused_where_it_stands() {
    let files = edited(
        COMPONENT,
        "qualityAttributes:\n",
        "requirements:\n  - key: 00000000-0000-7000-8000-000000000079\n    statement:\n      text: \"Any.\"\nqualityAttributes:\n",
    );
    let error = refused_in(&files, COMPONENT);
    assert_eq!(
        error.problem,
        Problem::UnknownField("requirements".to_owned())
    );
}

#[test]
fn ac_318_a_product_file_with_two_product_records_is_refused() {
    let product = text(&files(), PRODUCT);
    let two = format!("- {}", product.replace('\n', "\n  ").trim_end());
    let mut files = files();
    files.insert(path(PRODUCT), format!("{two}\n{two}\n").into_bytes());
    refused_in(&files, PRODUCT);
}

#[test]
fn ac_319_a_product_file_without_a_product_record_is_refused() {
    let mut files = files();
    files.insert(path(PRODUCT), Vec::new());
    refused_in(&files, PRODUCT);
}

#[test]
fn ac_322_a_realizes_edge_from_a_requirement_is_refused() {
    let files = edited(
        FEATURE,
        "    statement:\n",
        &format!("    realizes:\n      - {COMPONENT_KEY}\n    statement:\n"),
    );
    let error = refused_in(&files, FEATURE);
    assert_eq!(error.problem, Problem::UnknownField("realizes".to_owned()));
}

#[test]
fn ac_323_a_realizes_edge_to_a_requirement_is_refused() {
    let files = edited(
        FEATURE,
        &format!("  - {COMPONENT_KEY}"),
        &format!("  - {REQUIREMENT_KEY}"),
    );
    let error = refused_in(&files, FEATURE);
    assert_eq!(
        model_problem(&error),
        "the key names an element of another kind"
    );
}

#[test]
fn ac_325_a_scoped_to_edge_to_a_feature_is_refused() {
    let files = edited(
        COMPONENT,
        "  - 00000000-0000-7000-8000-000000000001",
        &format!("  - {FEATURE_KEY}"),
    );
    let error = refused_in(&files, COMPONENT);
    assert_eq!(
        model_problem(&error),
        "the key names an element of another kind"
    );
}

#[test]
fn ac_326_a_subject_edge_from_a_node_is_refused() {
    let files = edited(
        FEATURE,
        "requirements:\n",
        &format!("subjects:\n  - role: affected\n    node: {COMPONENT_KEY}\nrequirements:\n"),
    );
    let error = refused_in(&files, FEATURE);
    assert_eq!(error.problem, Problem::UnknownField("subjects".to_owned()));
}

#[test]
fn ac_327_a_subject_edge_to_a_decision_is_refused() {
    let files = edited(
        DECISION,
        &format!("    node: {COMPONENT_KEY}"),
        &format!("    node: {DECISION_KEY}"),
    );
    let error = refused_in(&files, DECISION);
    assert_eq!(
        model_problem(&error),
        "the key names an element of another kind"
    );
}

#[test]
fn ac_328_a_subject_edge_without_a_role_is_refused() {
    let files = edited(DECISION, "  - role: affected\n    node:", "  - node:");
    let error = refused_in(&files, DECISION);
    assert_eq!(error.problem, Problem::MissingField("role"));
}

#[test]
fn ac_328_a_subject_edge_with_another_role_is_refused() {
    let files = edited(DECISION, "role: affected", "role: mentioned");
    refused_in(&files, DECISION);
}

#[test]
fn ac_642_a_realizes_edge_that_carries_a_role_is_refused() {
    let files = edited(
        FEATURE,
        &format!("  - {COMPONENT_KEY}"),
        &format!("  - node: {COMPONENT_KEY}\n    role: outcome"),
    );
    refused_in(&files, FEATURE);
}

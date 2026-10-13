//! What the reader refuses in a `spec/` tree, and where it says the error is (F-6..F-10,
//! F-49, F-47).

mod common;

use common::text::{FEATURE, PRODUCT, edited, files, path};
use loftspec::codec::{self, Files, Problem, ReadError};
use loftspec::manifest::ManifestError;
use loftspec::yaml::{Location, Position};

fn refused(files: &Files) -> Vec<ReadError> {
    codec::read(files).expect_err("refused")
}

/// The one error a tree gives.
fn only(files: &Files) -> ReadError {
    let mut errors = refused(files);
    assert_eq!(errors.len(), 1, "{errors:?}");
    errors.remove(0)
}

fn at(line: usize, column: usize) -> Location {
    Location::At(Position::new(line, column))
}

#[test]
fn a_tree_without_a_manifest_is_refused_naming_its_path() {
    let mut files = files();
    files.remove(&path("loftspec.yaml"));
    let error = only(&files);
    assert_eq!(error.problem, Problem::MissingManifest);
    assert_eq!(
        error.to_string(),
        "spec/loftspec.yaml: the manifest is missing"
    );
}

#[test]
fn an_unknown_format_is_refused_and_no_other_file_is_read() {
    let mut files = edited("loftspec.yaml", "loftspec/1", "loftspec/2");
    files.insert(path("notes.yaml"), b"a: 1\n".to_vec());
    let error = only(&files);
    let unknown = ManifestError::UnknownFormat("loftspec/2".to_owned());
    assert_eq!(error.problem, Problem::Unsupported(unknown));
}

#[test]
fn an_unknown_field_is_refused_at_its_position() {
    let files = edited(FEATURE, "slug: keys\n", "slug: keys\ncolour: \"red\"\n");
    let error = only(&files);
    assert_eq!(error.problem, Problem::UnknownField("colour".to_owned()));
    assert_eq!(error.sites[0].location, Some(at(3, 1)));
    assert_eq!(
        error.to_string(),
        "spec/products/example-product/identity/keys.yaml:3:1: the field `colour` is not one the \
         Format defines here"
    );
}

#[test]
fn an_edge_of_another_type_is_refused() {
    let files = edited(FEATURE, "realizes:", "dependsOn:");
    assert_eq!(
        only(&files).problem,
        Problem::UnknownField("dependsOn".to_owned())
    );
}

#[test]
fn a_file_the_format_does_not_name_is_refused() {
    let mut files = files();
    files.insert(path("notes.yaml"), b"a: 1\n".to_vec());
    let error = only(&files);
    assert_eq!(error.problem, Problem::UnknownFile);
    assert_eq!(error.sites[0].path, path("notes.yaml"));
}

#[test]
fn a_record_without_a_key_is_refused_outside_the_formatter() {
    let files = edited(
        FEATURE,
        "      - key: 00000000-0000-7000-8000-000000000005\n        name:",
        "      - name:",
    );
    assert_eq!(only(&files).problem, Problem::MissingField("key"));
}

#[test]
fn a_key_in_uppercase_is_refused() {
    let files = edited(
        FEATURE,
        "key: 00000000-0000-7000-8000-00000000000a",
        "key: 00000000-0000-7000-8000-00000000000A",
    );
    assert!(matches!(only(&files).problem, Problem::Invalid { .. }));
}

#[test]
fn a_tag_name_with_an_uppercase_letter_is_refused() {
    let files = edited(PRODUCT, "  - portable\n", "  - Portable\n");
    let error = only(&files);
    assert!(matches!(
        error.problem,
        Problem::Invalid {
            expected: "a tag name",
            ..
        }
    ));
    assert_eq!(error.sites[0].location, Some(at(6, 5)));
}

#[test]
fn a_statement_with_text_and_a_form_is_refused() {
    let files = edited(
        FEATURE,
        "      form: ubiquitous\n",
        "      text: \"x\"\n      form: ubiquitous\n",
    );
    assert!(matches!(only(&files).problem, Problem::Shape(_)));
}

#[test]
fn a_missing_slot_is_refused_naming_it() {
    let files = edited(FEATURE, "form: ubiquitous", "form: event_driven");
    assert_eq!(only(&files).problem, Problem::MissingField("when"));
}

#[test]
fn a_slot_the_form_does_not_take_is_refused() {
    let files = edited(FEATURE, "      system:", "      when: \"x\"\n      system:");
    assert!(matches!(only(&files).problem, Problem::Shape(message) if message.contains("when")));
}

#[test]
fn a_slug_that_differs_from_its_file_name_is_refused() {
    let files = edited(FEATURE, "slug: keys", "slug: other");
    let error = only(&files);
    assert_eq!(
        error.problem,
        Problem::SlugMismatch {
            slug: "other".to_owned(),
            name: "keys".to_owned()
        }
    );
}

#[test]
fn a_feature_file_outside_its_groups_directory_is_refused() {
    let mut files = files();
    let bytes = files.remove(&path(FEATURE)).unwrap();
    files.insert(path("products/example-product/elsewhere/keys.yaml"), bytes);
    let error = only(&files);
    assert_eq!(
        error.problem,
        Problem::WrongDirectory("identity".to_owned())
    );
}

#[test]
fn a_listed_feature_without_a_file_is_refused() {
    let files = edited(
        PRODUCT,
        "      - 00000000-0000-7000-8000-000000000003\n",
        "      - 00000000-0000-7000-8000-000000000003\n      - 00000000-0000-7000-8000-000000000063\n",
    );
    let error = only(&files);
    assert!(matches!(error.problem, Problem::ListedWithoutFile(_)));
    assert_eq!(error.sites[0].location, Some(at(13, 9)));
}

#[test]
fn a_feature_file_no_group_lists_is_refused() {
    let mut files = files();
    let other = String::from_utf8(files[&path(FEATURE)].clone())
        .unwrap()
        .replace("000000000003", "000000000063")
        .replace("slug: keys", "slug: more")
        .replace("000000000004", "000000000064")
        .replace("000000000005", "000000000065")
        .replace("00000000000a", "00000000006a");
    files.insert(
        path("products/example-product/identity/more.yaml"),
        other.into_bytes(),
    );
    assert_eq!(only(&files).problem, Problem::NotListed("Feature Group"));
}

#[test]
fn a_feature_listed_twice_by_one_group_is_refused() {
    let files = edited(
        PRODUCT,
        "      - 00000000-0000-7000-8000-000000000003\n",
        "      - 00000000-0000-7000-8000-000000000003\n      - 00000000-0000-7000-8000-000000000003\n",
    );
    assert!(matches!(only(&files).problem, Problem::ListedTwice(_)));
}

#[test]
fn a_listed_product_without_a_file_is_refused() {
    let files = edited(
        "loftspec.yaml",
        "products:\n",
        "products:\n  - 00000000-0000-7000-8000-000000000063\n",
    );
    assert!(matches!(
        only(&files).problem,
        Problem::ListedWithoutFile(_)
    ));
}

#[test]
fn a_key_carried_twice_is_refused_naming_both_places() {
    let files = edited(
        "decisions/keys-are-uuids.yaml",
        "key: 00000000-0000-7000-8000-000000000019",
        "key: 00000000-0000-7000-8000-000000000003",
    );
    let error = only(&files);
    assert_eq!(error.sites.len(), 2);
    assert_ne!(error.sites[0].path, error.sites[1].path);
    assert!(matches!(error.problem, Problem::DuplicateKey(_)));
}

#[test]
fn an_empty_registry_file_reads_as_no_entries() {
    let mut files = files();
    files.insert(path("categories.yaml"), Vec::new());
    let files = {
        let mut files = files;
        let constraint = String::from_utf8(files[&path("components/document-model.yaml")].clone())
            .unwrap()
            .replace("    constraintBinding:\n      category: runtime.language\n      requiredValue: \"rust\"\n", "");
        files.insert(
            path("components/document-model.yaml"),
            constraint.into_bytes(),
        );
        files
    };
    assert_eq!(codec::read(&files).unwrap().categories, Vec::new());
}

#[test]
fn a_former_label_reads_as_a_former_label() {
    let files = edited(
        "decisions/keys-are-uuids.yaml",
        "    node: 00000000-0000-7000-8000-000000000006\n",
        "    node:\n      formerLabel: CMP-9\n",
    );
    let document = codec::read(&files).unwrap();
    assert!(document.edges.iter().any(|edge| matches!(
        edge,
        loftspec::document::Edge::Subject { subject: loftspec::decision::NodeRef::Former(label), .. }
            if label.to_string() == "CMP-9"
    )));
}

#[test]
fn an_empty_body_stays_empty() {
    let files = edited(PRODUCT, "body: \"The example.\"", "body: \"\"");
    let document = codec::read(&files).unwrap();
    assert_eq!(document.products[0].details.body.as_deref(), Some(""));
}

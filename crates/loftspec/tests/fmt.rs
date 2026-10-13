//! `loft fmt` as a function (F-39): canonical layout, minted keys, supplied slugs.

use crate::common::small_document;
use crate::common::text::{FEATURE, PRODUCT, edited, path, text};
use crate::common::v1::{convert_value, full_export};
use loftspec::codec::{self, Files, Problem};
use loftspec::fmt::format;
use loftspec::identity::Key;

/// The files after formatting.
fn formatted(files: &Files) -> Files {
    let mut files = files.clone();
    files.extend(format(&files).unwrap().changed);
    files
}

#[test]
fn a_canonical_tree_formats_to_itself() {
    for document in [
        small_document(),
        convert_value(&full_export()).unwrap().document,
    ] {
        let files = codec::write(&document);
        let result = format(&files).unwrap();
        assert!(result.changed.is_empty(), "{:?}", result.changed.keys());
        assert_eq!(result.minted, 0);
    }
}

#[test]
fn only_a_file_whose_bytes_change_is_rewritten() {
    let canonical = codec::write(&small_document());
    let files = edited(
        FEATURE,
        "name: \"Keys\"\nbody: \"Keys address nodes.\"\n",
        "body: Keys address nodes.\n\nname: Keys\n",
    );
    let result = format(&files).unwrap();
    assert_eq!(result.changed.keys().collect::<Vec<_>>(), [&path(FEATURE)]);
    assert_eq!(result.changed[&path(FEATURE)], canonical[&path(FEATURE)]);
}

#[test]
fn a_record_without_a_key_gets_a_version_7_key() {
    let files = edited(
        FEATURE,
        "      - key: 00000000-0000-7000-8000-000000000005\n        name:",
        "      - name:",
    );
    let result = format(&files).unwrap();
    assert_eq!(result.minted, 1);
    let written = String::from_utf8(result.changed[&path(FEATURE)].clone()).unwrap();
    let criterion_key = written
        .split("criteria:")
        .nth(1)
        .and_then(|rest| {
            rest.lines()
                .find_map(|line| line.trim().strip_prefix("- key: "))
        })
        .map(|key| Key::parse(key).unwrap())
        .unwrap();
    assert_eq!(criterion_key.version(), 7);
    assert!(codec::read(&formatted(&files)).is_ok());
}

#[test]
fn two_keys_minted_in_one_run_differ() {
    let files = edited(
        "decisions/keys-are-uuids.yaml",
        "  - key: 00000000-0000-7000-8000-000000000017\n    title:",
        "  - title:",
    );
    let files = {
        let mut files = files;
        let current = text(&files, "decisions/keys-are-uuids.yaml");
        let changed = current.replacen(
            "  - key: 00000000-0000-7000-8000-000000000018\n    title:",
            "  - title:",
            1,
        );
        files.insert(path("decisions/keys-are-uuids.yaml"), changed.into_bytes());
        files
    };
    let result = format(&files).unwrap();
    assert_eq!(result.minted, 2);
}

#[test]
fn an_option_without_a_key_gets_one() {
    let files = edited(
        "decisions/keys-are-uuids.yaml",
        "  - key: 00000000-0000-7000-8000-000000000018\n    title:",
        "  - title:",
    );
    assert_eq!(format(&files).unwrap().minted, 1);
}

#[test]
fn a_read_error_formats_nothing() {
    let files = edited(FEATURE, "name: \"Keys\"", "name: \"Keys\" # note");
    let errors = format(&files).unwrap_err();
    assert_eq!(errors[0].sites[0].path, path(FEATURE));
}

#[test]
fn a_feature_without_a_slug_takes_its_file_name() {
    let files = edited(FEATURE, "slug: keys\n", "");
    let result = format(&files).unwrap();
    assert!(text(&result.changed, FEATURE).contains("slug: keys\n"));
}

#[test]
fn a_feature_group_without_a_slug_takes_its_directory() {
    let files = edited(PRODUCT, "    slug: identity\n", "");
    let result = format(&files).unwrap();
    assert!(text(&result.changed, PRODUCT).contains("    slug: identity\n"));
}

#[test]
fn a_renamed_node_keeps_its_file() {
    let files = edited(FEATURE, "name: \"Keys\"", "name: \"Node keys\"");
    let result = format(&files).unwrap();
    assert!(result.changed.is_empty());
    let document = codec::read(&files).unwrap();
    assert_eq!(
        document.products[0].feature_groups[0].features[0].name,
        "Node keys"
    );
}

#[test]
fn a_new_feature_files_key_is_appended_to_the_features_of_its_directorys_group() {
    let mut files = codec::write(&small_document());
    files.insert(
        path("products/example-product/identity/labels.yaml"),
        b"name: \"Labels\"\n".to_vec(),
    );
    let files = formatted(&files);
    let document = codec::read(&files).unwrap_or_else(|errors| panic!("{errors:?}"));
    let features = &document.products[0].feature_groups[0].features;
    assert_eq!(features.len(), 2);
    assert_eq!(features[1].name, "Labels");
    assert_eq!(features[1].slug.as_str(), "labels");
}

#[test]
fn a_new_feature_file_no_group_can_list_is_refused_and_nothing_is_formatted() {
    let mut files = codec::write(&small_document());
    let stray = path("products/example-product/elsewhere/labels.yaml");
    files.insert(stray.clone(), b"name: \"Labels\"\n".to_vec());
    let errors = format(&files).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].sites[0].path, stray);
    assert_eq!(errors[0].problem, Problem::NotListed("Feature Group"));
}

#[test]
fn a_new_product_files_key_is_appended_to_the_manifests_products() {
    let mut files = codec::write(&small_document());
    files.insert(
        path("products/new-product/product.yaml"),
        b"name: \"New Product\"\n".to_vec(),
    );
    let files = formatted(&files);
    let document = codec::read(&files).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(document.products.len(), 2);
    assert_eq!(document.products[0].name, "Example Product");
    assert_eq!(document.products[1].name, "New Product");
    assert_eq!(document.products[1].slug.as_str(), "new-product");
}

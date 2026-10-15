//! A Product or Feature the writer is handed without a key is not lost: its file is written
//! without a key and left off its parent's list, the reader refuses that file, and the formatter
//! mints the key and lists the file, after which the node reads back.

use crate::common::small_document;
use crate::common::text::{path, text};
use crate::fmt::formatted;
use loftspec::codec::{self, Files, Problem};
use loftspec::document::{Document, Edge};

/// A document can name a node in an edge only by its key, so the fixtures drop the edges that
/// name the node whose key they clear.
fn without_edges_naming(document: &mut Document, keep: impl Fn(&Edge) -> bool) {
    document.edges.retain(keep);
}

/// The names of every Product and Feature in `document`.
fn names(document: &Document) -> Vec<String> {
    let mut names = Vec::new();
    for product in &document.products {
        names.push(product.name.clone());
        for group in &product.feature_groups {
            names.extend(group.features.iter().map(|feature| feature.name.clone()));
        }
    }
    names
}

fn without_product_key() -> Document {
    let mut document = small_document();
    document.products[0].id = None;
    without_edges_naming(&mut document, |edge| !matches!(edge, Edge::ScopedTo { .. }));
    document
}

fn without_feature_key() -> Document {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0].id = None;
    without_edges_naming(&mut document, |edge| !matches!(edge, Edge::Realizes { .. }));
    document
}

const PRODUCT_FILE: &str = "products/example-product/product.yaml";
const FEATURE_FILE: &str = "products/example-product/identity/keys.yaml";

/// Asserts the written tree holds `file` without a key and that the reader refuses it, naming
/// the file and the missing key.
fn assert_written_and_refused(files: &Files, file: &str) {
    assert!(
        !text(files, file).starts_with("key:"),
        "{file} holds no key"
    );
    let errors = codec::read(files).expect_err("a keyless file is refused");
    assert!(
        errors.iter().any(|error| {
            error.problem == Problem::MissingField("key")
                && error.sites.iter().any(|site| site.path == path(file))
        }),
        "{errors:?}"
    );
}

#[test]
fn a_product_without_a_key_is_written_and_refused_on_reading() {
    let files = codec::write(&without_product_key());
    assert_written_and_refused(&files, PRODUCT_FILE);
    assert!(!text(&files, "loftspec.yaml").contains("products:"));
}

#[test]
fn a_feature_without_a_key_is_written_and_refused_on_reading() {
    let document = without_feature_key();
    let files = codec::write(&document);
    assert_written_and_refused(&files, FEATURE_FILE);
    assert!(!text(&files, PRODUCT_FILE).contains("features:"));
}

#[test]
fn the_formatter_recovers_every_node_written_without_a_key() {
    for document in [without_product_key(), without_feature_key()] {
        let back = codec::read(&formatted(&codec::write(&document)))
            .unwrap_or_else(|errors| panic!("{errors:?}"));
        assert_eq!(names(&back), names(&document));
    }
}

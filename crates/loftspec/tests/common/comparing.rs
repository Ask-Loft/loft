//! Two versions of a document compared, neither holding `spec/lint.yaml`.

use loftspec::diff::{Comparison, Version, compare};
use loftspec::document::Document;

/// A version without `spec/lint.yaml`.
pub fn version(document: &Document) -> Version<'_> {
    Version {
        document,
        lint: None,
    }
}

/// The comparison of `older` with `newer`, neither holding `spec/lint.yaml`.
pub fn compared(older: &Document, newer: &Document) -> Comparison {
    compare(version(older), version(newer)).unwrap()
}

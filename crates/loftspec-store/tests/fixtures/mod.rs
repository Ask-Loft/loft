//! What the store tests that save documents share: converted v1 exports, and a scratch directory
//! whose `spec/` holds a lint configuration the document does not.

use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;
use loftspec::document::Document;

use crate::common::Scratch;

/// A lint configuration the document does not hold, which every save must leave alone.
pub const LINT_CONFIG: &str = "rules:\n  - rule: SH-002\n    enabled: false\n";

/// The document a v1 export converts to.
pub fn converted(export: &str) -> Document {
    convert(&Export::from_json(export).unwrap())
        .unwrap()
        .document
}

/// A fresh directory whose `spec/` holds only [`LINT_CONFIG`].
pub fn with_lint() -> Scratch {
    let scratch = Scratch::new();
    scratch.put("spec/lint.yaml", LINT_CONFIG);
    scratch
}

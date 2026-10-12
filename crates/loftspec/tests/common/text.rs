//! The small document's `spec/` tree, for the reader and formatter tests.

use loftspec::codec::{self, Files, SpecPath};

use super::small_document;

pub const FEATURE: &str = "products/example-product/identity/keys.yaml";
pub const PRODUCT: &str = "products/example-product/product.yaml";

/// The small document's files.
pub fn files() -> Files {
    codec::write(&small_document())
}

pub fn path(text: &str) -> SpecPath {
    SpecPath::new(text)
}

/// A file's text.
pub fn text(files: &Files, at: &str) -> String {
    String::from_utf8(files[&path(at)].clone()).unwrap()
}

/// The small document's files with the first `from` replaced by `to` in the file at `at`.
pub fn edited(at: &str, from: &str, to: &str) -> Files {
    let mut files = files();
    let current = text(&files, at);
    assert!(current.contains(from), "{from:?} in {at}");
    files.insert(path(at), current.replacen(from, to, 1).into_bytes());
    files
}

/// The last line of the Requirement's criteria, after which a field of the Requirement goes.
pub const AFTER_CRITERIA: &str = "          - 00000000-0000-7000-8000-000000000004\n";

/// A conflict record over the Requirement's `statement.shall`.
pub const SHALL_CONFLICT: &str = "    conflicts:
      - field: statement.shall
        base: \"write a key on every node.\"
        ours: \"write a key on each node.\"
        theirs: \"write one key on every node.\"
";

/// The small document with `lines` added to its Requirement after every other field.
pub fn on_requirement(lines: &str) -> Files {
    edited(FEATURE, AFTER_CRITERIA, &format!("{AFTER_CRITERIA}{lines}"))
}

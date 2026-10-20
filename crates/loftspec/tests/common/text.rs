//! The small document's `spec/` tree, for the reader and formatter tests.

use loftspec::codec::{self, Files, ReadError, SpecPath};
use loftspec::fmt::format;

use super::small_document;

pub const FEATURE: &str = "products/example-product/identity/keys.yaml";
pub const PRODUCT: &str = "products/example-product/product.yaml";
pub const COMPONENT: &str = "components/document-model.yaml";
pub const DECISION: &str = "decisions/keys-are-uuids.yaml";

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

/// The one error a tree gives, which must name the file at `at`.
pub fn refused_in(files: &Files, at: &str) -> ReadError {
    let errors = codec::read(files).expect_err("refused");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].sites[0].path, path(at), "{errors:?}");
    errors[0].clone()
}

/// The small document's files with the first `from` replaced by `to` in the file at `at`.
pub fn edited(at: &str, from: &str, to: &str) -> Files {
    let mut files = files();
    let current = text(&files, at);
    assert!(current.contains(from), "{from:?} in {at}");
    files.insert(path(at), current.replacen(from, to, 1).into_bytes());
    files
}

/// The files with the first `from` replaced by `to` in the file at `at`.
pub fn replaced(files: &Files, at: &str, from: &str, to: &str) -> Files {
    let current = text(files, at);
    assert!(current.contains(from), "{from:?} in {at}");
    let mut files = files.clone();
    files.insert(path(at), current.replacen(from, to, 1).into_bytes());
    files
}

/// The files with the text between `from` and `to` in the file at `at` removed, `from`
/// included and `to` kept.
pub fn cut(files: &Files, at: &str, from: &str, to: &str) -> Files {
    let current = text(files, at);
    let start = current
        .find(from)
        .unwrap_or_else(|| panic!("{from:?} in {at}"));
    let end = start
        + current[start..]
            .find(to)
            .unwrap_or_else(|| panic!("{to:?} in {at}"));
    let mut files = files.clone();
    let edited = format!("{}{}", &current[..start], &current[end..]);
    files.insert(path(at), edited.into_bytes());
    files
}

/// The files after formatting.
pub fn formatted(files: &Files) -> Files {
    let mut files = files.clone();
    files.extend(format(&files).unwrap().changed);
    files
}

/// The tree `files` with the Constraint `CON-1` deleted from its Component.
pub fn without_constraint(files: &Files) -> Files {
    cut(files, COMPONENT, "constraints:\n", "qualityAttributes:\n")
}

/// The small document's tree with the Requirement `RQ-2` and its criterion deleted from their
/// Feature, and the decision that names it in the state `kind`, with any lines that state needs
/// after it.
pub fn without_requirement(kind: &str) -> Files {
    let files = cut(&files(), FEATURE, "requirements:\n", "qualityAttributes:\n");
    replaced(
        &files,
        DECISION,
        "  kind: accepted\n",
        &format!("  kind: {kind}\n"),
    )
}

/// `files` with `spec/lint.yaml` holding `lint`, or without the file.
pub fn linted(files: &Files, lint: Option<&str>) -> Files {
    let mut files = files.clone();
    let path = SpecPath::new(codec::LINT);
    match lint {
        Some(lint) => files.insert(path, lint.as_bytes().to_vec()),
        None => files.remove(&path),
    };
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

/// The small document's files with the Feature's body written as the block scalar `body`.
pub fn with_feature_body(body: &str) -> Files {
    edited(
        FEATURE,
        "body: \"Keys address nodes.\"\n",
        &format!("body: |\n{}", indented(body, "  ")),
    )
}

/// `text` with every line indented by `by`.
pub fn indented(text: &str, by: &str) -> String {
    text.lines().flat_map(|line| [by, line, "\n"]).collect()
}

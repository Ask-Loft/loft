//! The text form (D-11 FG-2): a `spec/` tree read into a document and a document written back
//! as files in canonical layout. The writer yields paths and bytes; writing them to disk
//! belongs to a store (CMP-4).

mod canon;
mod error;
mod files;
mod layout;
mod record;
mod resolve;
mod resolve_decisions;
mod resolve_nodes;
pub mod schema;
mod unresolve;
mod unresolve_parts;
mod variants;

pub use error::{Problem, ReadError, Site, SpecPath};
pub use files::{CATEGORIES, Content, Files, LABELS, LINT, MANIFEST, Role, TAGS, Tree};
pub use layout::read_tree;
pub(crate) use layout::{check_tree, parse_files};
pub use record::{Mode, Rec, Val};
pub use resolve::resolve;
pub use unresolve::unresolve;

pub(crate) use canon::canonicalize;
pub(crate) use files::write_files;

use crate::document::Document;

/// Reads a `spec/` tree into a document, as every role but the formatter does.
///
/// # Errors
///
/// Every read error the tree holds (RQ-30..RQ-33, RQ-65, RQ-4), or the first reference the
/// model cannot hold.
pub fn read(files: &Files) -> Result<Document, Vec<ReadError>> {
    let tree = read_tree(files)?;
    resolve(&tree).map_err(|error| vec![error])
}

/// Writes a document as the files of its `spec/` tree, in canonical layout.
#[must_use]
pub fn write(document: &Document) -> Files {
    let mut tree = unresolve(document);
    canonicalize(&mut tree);
    write_files(&tree)
}

//! The text form (FG-2): a `spec/` tree read into a document and a document written back
//! as files in canonical layout. The writer yields paths and bytes; writing them to disk
//! belongs to a store (CMP-4).

mod canon;
mod conflict;
mod error;
pub mod field;
mod files;
mod layout;
mod record;
mod resolve;
mod resolve_conflicts;
mod resolve_decisions;
mod resolve_nodes;
pub mod schema;
mod strict;
mod unresolve;
mod unresolve_parts;
mod variants;

pub use conflict::{SideShape, side_shape};
pub use error::{Problem, ReadError, Site, SpecPath, write_sites};
pub(crate) use files::keyed_records;
pub use files::{CATEGORIES, Content, Files, LABELS, LINT, MANIFEST, Role, TAGS, Tree};
pub use layout::{Sites, read_tree};
pub(crate) use layout::{check_tree, parse_files};
pub use record::{ConflictRec, Mode, Rec, Val};
pub use resolve::resolve;
pub use unresolve::unresolve;

pub(crate) use canon::canonicalize;
pub(crate) use files::{read_file, write_files};
pub(crate) use resolve_nodes::statement_of;
pub(crate) use strict::Strict;

use crate::document::Document;

/// Reads a `spec/` tree into a document, as every role but the formatter does.
///
/// # Errors
///
/// Every read error the tree holds (RQ-30..RQ-33, RQ-65, RQ-4), or else, in each file, the
/// first reference the model cannot hold.
pub fn read(files: &Files) -> Result<Document, Vec<ReadError>> {
    read_sited(files).map(|(document, _)| document)
}

/// Reads a `spec/` tree into a document, with the site of every key it carries.
///
/// # Errors
///
/// As [`read`].
pub fn read_sited(files: &Files) -> Result<(Document, Sites), Vec<ReadError>> {
    let tree = read_tree(files)?;
    let document = resolve(&tree)?;
    Ok((document, Sites::of(&tree)))
}

/// Writes a document as the files of its `spec/` tree, in canonical layout.
#[must_use]
pub fn write(document: &Document) -> Files {
    let mut tree = unresolve(document);
    canonicalize(&mut tree);
    write_files(&tree)
}

/// One file's content in canonical layout, exactly as [`write`] writes that file.
pub(crate) fn write_content(path: &SpecPath, role: Role, content: Content) -> Vec<u8> {
    let mut tree = Tree::default();
    tree.files.insert(path.clone(), (role, content));
    canonicalize(&mut tree);
    write_files(&tree)
        .remove(path)
        .expect("the writer writes the file")
}

/// Whether a record holds exactly the fields its variant takes (RQ-220's variants).
pub(crate) fn variant_holds(record: &Rec) -> bool {
    variants::check(record).is_ok()
}

/// The bytes of `spec/labels.yaml` alone, exactly as [`write`] writes them, without writing the
/// rest of the tree.
#[must_use]
pub fn write_labels(document: &Document) -> Vec<u8> {
    let mut tree = unresolve::unresolve_labels(document);
    canonicalize(&mut tree);
    write_files(&tree)
        .remove(&SpecPath::new(LABELS))
        .expect("the writer writes the bindings")
}

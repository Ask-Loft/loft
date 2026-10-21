//! The text form (FG-2): a `spec/` tree read into a document and a document written back
//! as files in canonical layout. The writer yields paths and bytes; writing them to disk
//! belongs to a store (CMP-4).

mod canon;
mod conflict;
mod error;
mod ext;
pub mod field;
mod files;
mod held;
mod layout;
mod node_files;
mod own_record;
mod path;
mod record;
mod records;
mod resolve;
mod resolve_conflicts;
mod resolve_decisions;
mod resolve_nodes;
pub mod schema;
mod settle_field;
mod strict;
mod unresolve;
mod unresolve_one;
mod unresolve_parts;
mod unresolve_singles;
mod variants;

pub use conflict::{SideShape, side_shape};
pub use error::{Problem, ReadError, Site, SpecPath, site_fields, write_sites};
pub(crate) use files::keyed_records;
pub use files::{
    CATEGORIES, Content, Files, LABELS, LINT, MANIFEST, Role, TAGS, TERMS, Tree, lint_configuration,
};
pub(crate) use held::write_and_read;
pub use layout::{Sites, read_tree};
pub(crate) use layout::{check_tree, parse_files};
pub(crate) use node_files::{Under, filed_in_order, listed, node_files};
pub use own_record::{OwnRecord, RecordKind, own_record, own_record_digest, own_records};
pub(crate) use path::holder_path;
pub use record::{ConflictRec, Mode, Rec, Val};
pub(crate) use records::records;
pub use resolve::resolve;
pub use resolve_conflicts::conflict_owner;
pub(crate) use settle_field::{FieldRefusal, settle_field};
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
    write_files(&canonical_tree(document))
}

/// The tree of a document's records in canonical layout, as the writer writes them.
pub(crate) fn canonical_tree(document: &Document) -> Tree {
    let mut tree = unresolve(document);
    canonicalize(&mut tree);
    tree
}

/// Writes a document as [`write`] does, with the site of every key its files carry, as reading
/// those files back gives it, without reading them.
#[must_use]
pub fn write_sited(document: &Document) -> (Files, Sites) {
    let (_, files, sites) = written(document);
    (files, sites)
}

/// A document's canonical tree, the files the writer writes from it and the sites of the keys
/// they carry.
pub(crate) fn written(document: &Document) -> (Tree, Files, Sites) {
    let tree = canonical_tree(document);
    let (files, keys) = files::write_files_keyed(&tree);
    let sites = Sites::written(keys);
    (tree, files, sites)
}

/// The site of every key the files the writer writes for a document carry, as reading those
/// files back gives it, without reading them.
#[must_use]
pub fn sites(document: &Document) -> Sites {
    write_sited(document).1
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

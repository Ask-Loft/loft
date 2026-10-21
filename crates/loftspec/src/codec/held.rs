//! A document's written tree read without parsing its files: the records the writer builds are
//! the records the strict reader would read from the files it writes, but for their positions,
//! so where they hold only what that reader accepts they are checked and resolved as they are,
//! and the sites of their keys come from the writer. Otherwise the files are read, so that every
//! error is the reader's own, at its site.

use super::canon::canonicalize;
use super::error::ReadError;
use super::files::{Files, keyed_records, write_files_keyed};
use super::layout::{Sites, check_tree};
use super::read_sited;
use super::resolve::resolve;
use super::unresolve::unresolve;
use crate::document::Document;

/// The document and the sites reading a tree gives, or every read error it holds.
pub(crate) type Read = Result<(Document, Sites), Vec<ReadError>>;

/// The files the writer writes for `document`, and what reading them back gives.
pub(crate) fn write_and_read(document: &Document) -> (Files, Read) {
    let mut tree = unresolve(document);
    canonicalize(&mut tree);
    let (files, keys) = write_files_keyed(&tree);
    let held = strictly_held(document, &tree)
        .then(|| check_tree(&tree).and_then(|()| resolve(&tree)))
        .and_then(Result::ok);
    let read = match held {
        Some(read) => Ok((read, Sites::written(keys))),
        None => read_sited(&files),
    };
    (files, read)
}

/// Whether the writer's records for `document` hold only what the strict reader accepts of a
/// field's value, beyond what the document's types ensure: a key on every keyed record, a publish
/// branch that is a one-line identifier, a positive publish sequence, and extension data the
/// text form holds.
fn strictly_held(document: &Document, tree: &super::files::Tree) -> bool {
    let records = tree
        .files
        .values()
        .flat_map(|(_, content)| content.records());
    let keyed = keyed_records(records)
        .iter()
        .all(|record| record.key().is_some());
    let branch = document
        .publish_branch
        .as_deref()
        .is_none_or(|branch| !branch.is_empty() && !branch.contains('\n'));
    let sequence = document
        .derived_from
        .as_ref()
        .is_none_or(|source| source.publish_sequence > 0);
    let mut ext = true;
    document.walk(&mut |node| {
        ext &= crate::ext::from_json(&crate::ext::to_json(node.ext())).is_ok();
    });
    keyed && branch && sequence && ext
}

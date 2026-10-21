//! A document's written tree read without parsing its files: the records the writer builds are
//! the records the strict reader would read from the files it writes, but for their positions,
//! so where they hold only what that reader accepts they are checked and resolved as they are,
//! and the sites of their keys come from the writer; a file whose records hold conflict records
//! is read on its own, since the reader checks a conflict's field and sides. Otherwise the files
//! are read, so that every error is the reader's own, at its site.

use super::error::ReadError;
use super::files::{Files, Tree, keyed_records, read_file};
use super::layout::{Sites, check_tree};
use super::record::{Mode, Rec};
use super::resolve::resolve;
use super::{read_sited, written};
use crate::document::Document;

/// The document and the sites reading a tree gives, or every read error it holds.
pub(crate) type Read = Result<(Document, Sites), Vec<ReadError>>;

/// The files the writer writes for `document`, and what reading them back gives.
pub(crate) fn write_and_read(document: &Document) -> (Files, Read) {
    let (mut tree, files, sites) = written(document);
    let held = (strictly_held(document, &tree) && read_conflicted(&mut tree, &files))
        .then(|| check_tree(&tree).and_then(|()| resolve(&tree)))
        .and_then(Result::ok);
    let read = match held {
        Some(read) => Ok((read, sites)),
        None => read_sited(&files),
    };
    (files, read)
}

/// Whether the writer's records for `document` hold only what the strict reader accepts of a
/// field's value, beyond what the document's types ensure: a key on every keyed record, a publish
/// branch that is a one-line identifier, a positive publish sequence, and extension data the
/// text form holds.
fn strictly_held(document: &Document, tree: &Tree) -> bool {
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
        ext = ext && crate::ext::held(node.ext());
    });
    keyed && branch && sequence && ext
}

/// Reads on its own each written file whose records hold a conflict record, in place of the
/// writer's records: the reader checks a conflict's field and the shape of its sides, which the
/// document's types leave open, and reads a side as its field's shape reads it. Whether each
/// such file read.
fn read_conflicted(tree: &mut Tree, files: &Files) -> bool {
    for (path, file) in &mut tree.files {
        if !file.1.records().any(holds_conflicts) {
            continue;
        }
        match files
            .get(path)
            .map(|bytes| read_file(path, bytes, Mode::Strict))
        {
            Some(Ok(read)) => *file = read,
            _ => return false,
        }
    }
    true
}

/// Whether `record` or a record inside it holds a conflict record.
fn holds_conflicts(record: &Rec) -> bool {
    !record.conflicts.is_empty() || record.children().any(holds_conflicts)
}

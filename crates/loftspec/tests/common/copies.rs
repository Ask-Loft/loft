//! A connected document that holds a copy of another document's Interface, for the tests of
//! other documents, copies and references into them.

use loftspec::canonical::Digest;
use loftspec::codec::{self, copy_of};
use loftspec::copy::Copy;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec::publish::allocate;

use super::full::other_document;
use super::interfaces::{INTERFACE, with_interface};
use super::{key, slug, small_document};

/// The other document's key number.
pub const OWNER: u32 = 0x300;

/// The handle under which [`requiring`] lists the other document.
pub const HANDLE: &str = "format";

/// The path of the copy [`requiring`] holds, under `spec/`.
pub const COPY: &str = "documents/format/document-lookup.yaml";

/// The other document: the small document with its Interface "Document lookup", under a key of
/// its own, every node labeled as a publish labels it.
pub fn owner() -> Document {
    let mut document = with_interface();
    document.id = DocumentId::new(key(OWNER));
    allocate(&mut document);
    document
}

/// A state root standing for the other document's published state `sequence`.
pub fn state_root(sequence: u64) -> Digest {
    format!("{sequence:02x}").repeat(32).parse().unwrap()
}

/// The copy of the other document's Interface at `sequence`, under `handle`.
pub fn copy_at(owner: &Document, handle: &str, sequence: u64) -> Copy {
    let files = codec::write(owner);
    copy_of(
        &slug(handle),
        &files,
        key(INTERFACE),
        sequence,
        state_root(sequence),
    )
    .unwrap()
}

/// The small document, connected, listing the other document under [`HANDLE`] and holding a
/// copy of its Interface at publish sequence 3.
pub fn requiring() -> Document {
    let mut document = small_document();
    document.connected = true;
    document.remote = Some("https://remote.example/documents/one".to_owned());
    document.documents.push(other_document(HANDLE, OWNER));
    document.copies.push(copy_at(&owner(), HANDLE, 3));
    document
}

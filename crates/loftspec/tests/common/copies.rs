//! A connected document that holds a copy of another document's Interface, for the tests of
//! other documents, copies and references into them.

use loftspec::canonical::Digest;
use loftspec::codec::{self, copy_of};
use loftspec::copy::Copy;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec::publish::allocate;

use super::full::other_document;
use super::interfaces::{INTERFACE, interface, with_interface};
use super::{criterion, key, requirement, slug, small_document};

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

/// The second other document, listed under "catalog": its Interface "Shelf lookup" keyed
/// `0x2c0`, with a Requirement and its criterion, every node labeled.
pub fn second_owner() -> Document {
    let mut document = small_document();
    document.id = DocumentId::new(key(OWNER + 1));
    let promise = requirement(
        key(SHELF + 1),
        "The lookup shall answer a shelf by its name.",
        vec![criterion(key(SHELF + 2), "The answer names the shelf.")],
    );
    document.interfaces = vec![interface(key(SHELF), "Shelf lookup", vec![promise])];
    allocate(&mut document);
    document
}

/// The key number of [`second_owner`]'s Interface.
pub const SHELF: u32 = 0x2c0;

/// [`requiring`] listing [`second_owner`] as well, under "catalog", with a copy of its Interface.
pub fn requiring_two() -> Document {
    let mut document = requiring();
    document
        .documents
        .push(other_document("catalog", OWNER + 1));
    let files = codec::write(&second_owner());
    let copy = copy_of(&slug("catalog"), &files, key(SHELF), 7, state_root(7)).unwrap();
    document.copies.push(copy);
    document
}

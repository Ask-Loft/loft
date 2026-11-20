//! A tree that holds a copy of another document's Interface: the Reading List, connected,
//! listing another document under [`HANDLE`] and holding a copy of that document's Interface
//! "Shelf lookup", as that document published it at publish sequence 2.

use std::fs;

use loftspec::codec::{self, copy_of};
use loftspec::identity::Key;
use loftspec::ids::DocumentId;
use loftspec::publish::allocate;
use loftspec::slug::Slug;

use super::others::{OWN_REMOTE, list};
use super::signed_in::name_remote;
use super::{Scratch, tree};
use crate::interfaces::{LOOKUP, add_lookup};

/// The handle the tree lists the other document under.
pub const HANDLE: &str = "shelves";
/// The other document's key.
pub const OTHER: &str = "00000000-0000-4000-8000-0000000003ff";
/// The address of the other document's remote.
pub const OTHER_REMOTE: &str = "http://127.0.0.1:9/documents/shelves";

/// Writes into the tree of `scratch` a copy of the other document's "Shelf lookup", listing the
/// other document and marking the tree connected.
pub fn copy_lookup(scratch: &Scratch) {
    let owner = Scratch::with_reading_list();
    add_lookup(&owner);
    let mut published = codec::read(&tree(&owner.0)).unwrap();
    published.id = DocumentId::new(Key::parse(OTHER).unwrap());
    allocate(&mut published);
    let lookup = Key::parse(LOOKUP).unwrap();
    let state_root = "02".repeat(32).parse().unwrap();
    let handle = Slug::parse(HANDLE).unwrap();
    let copy = copy_of(&handle, &codec::write(&published), lookup, 2, state_root).unwrap();
    name_remote(scratch, Some(OWN_REMOTE), true);
    list(scratch, OTHER_REMOTE);
    let target = scratch.file(copy.path().as_str());
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, &copy.bytes).unwrap();
}

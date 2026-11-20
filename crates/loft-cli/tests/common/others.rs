//! Another document and its remote, for the tests of `loft interface`: the Reading List with the
//! Interface "Shelf lookup", under its own key, published twice, the second time with the
//! lookup's criterion reworded; and the tree that lists it under [`HANDLE`].

use std::fs;

use loftspec::codec::{self, MANIFEST, SpecPath};
use loftspec::document::OtherDocument;
use loftspec::identity::Key;
use loftspec::ids::DocumentId;
use loftspec::slug::Slug;
use serde_json::json;
use software_plant::remote::{Remote, RemoteScenario};

use super::copies::{HANDLE, OTHER};
use super::remote::{History, publish_at, stored};
use super::signed_in::{granted, keep, name_remote};
use super::{EPOCH, Scratch, tree};
use crate::interfaces::add_lookup;

/// The address of the tree's own remote, where nothing answers.
pub const OWN_REMOTE: &str = "http://127.0.0.1:9/documents/reading-list";
/// The path of the copy of the lookup under `spec/`.
pub const COPY: &str = "documents/shelves/shelf-lookup.yaml";

/// The other document published twice.
pub fn shelves() -> History {
    let owner = Scratch::with_reading_list();
    add_lookup(&owner);
    let mut document = codec::read(&tree(&owner.0)).unwrap();
    document.id = DocumentId::new(Key::parse(OTHER).unwrap());
    let files = codec::write(&document);
    let (first, files) = publish_at(&files, None, EPOCH + 60);
    let mut states = vec![stored(&first, &files)];
    let reworded = (files.iter())
        .map(|(path, bytes)| {
            let text = String::from_utf8(bytes.clone()).unwrap();
            let text = text.replace(
                "The returned book has the title asked for.",
                "The returned book has the title and the author asked for.",
            );
            (path.clone(), text.into_bytes())
        })
        .collect();
    let (second, files) = publish_at(&reworded, Some(&first), EPOCH + 120);
    states.push(stored(&second, &files));
    History {
        key: OTHER.to_owned(),
        states,
    }
}

/// A remote of the other document that holds `history`.
pub fn shelves_remote(history: &History) -> Remote {
    Remote::start(
        RemoteScenario::accepting(&history.key).holding("Shelves", history.states.clone()),
    )
}

/// Marks the tree of `scratch` connected to [`OWN_REMOTE`] and, where `listed`, lists the other
/// document under [`HANDLE`] at the address of `remote`; keeps a sign-in to `remote`.
pub fn listing(scratch: &Scratch, remote: &Remote, listed: bool) {
    name_remote(scratch, Some(OWN_REMOTE), true);
    if listed {
        list(scratch, &remote.address());
    }
    keep(scratch, &json!({ remote.address(): granted(remote, 3600) }));
}

/// Lists the other document under [`HANDLE`] at `address` in the manifest of `scratch`.
pub fn list(scratch: &Scratch, address: &str) {
    let mut document = codec::read(&tree(&scratch.0)).unwrap();
    document.documents.push(OtherDocument {
        handle: Slug::parse(HANDLE).unwrap(),
        key: DocumentId::new(Key::parse(OTHER).unwrap()),
        remote: address.to_owned(),
    });
    let manifest = SpecPath::new(MANIFEST);
    fs::write(scratch.file(MANIFEST), &codec::write(&document)[&manifest]).unwrap();
}

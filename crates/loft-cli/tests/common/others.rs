//! Another document and its remote, for the tests of `loft interface`: the Reading List with the
//! Interface "Shelf lookup", under its own key, published twice, the second time with the
//! lookup's criterion reworded; and the tree that lists it under [`HANDLE`].

use std::fs;

use loftspec::codec::{self, MANIFEST, SpecPath};
use loftspec::document::OtherDocument;
use loftspec::identity::Key;
use loftspec::ids::{CriterionId, DocumentId};
use loftspec::publish::{Lock, publish_labels};
use loftspec::slug::Slug;
use serde_json::json;
use software_plant::remote::{Remote, RemoteScenario};

use super::copies::{HANDLE, OTHER};
use super::remote::{History, publish_at, spec, stored};
use super::signed_in::{granted, keep, name_remote};
use super::{EPOCH, Scratch, tree};
use crate::interfaces::add_lookup;

/// The address of the tree's own remote, where nothing answers.
pub const OWN_REMOTE: &str = "http://127.0.0.1:9/documents/reading-list";
/// The path of the copy of the lookup under `spec/`.
pub const COPY: &str = "documents/shelves/shelf-lookup.yaml";

/// The lookup's criterion that the second state drops.
pub const DROPPED: &str = "00000000-0000-4000-8000-0000000000b3";
/// The lookup's criterion that the second state adds.
pub const ADDED: &str = "00000000-0000-4000-8000-0000000000b4";
/// The Interface "Shelf stock", which the second state withdraws.
pub const STOCK: &str = "00000000-0000-4000-8000-0000000000c0";

/// The other document published twice. The first state holds "Shelf lookup", `IF-1`, with its
/// answer and [`DROPPED`], and "Shelf stock", `IF-2`; the second rewords the answer, drops
/// [`DROPPED`], adds [`ADDED`] and withdraws "Shelf stock".
pub fn shelves() -> History {
    let owner = Scratch::with_reading_list();
    add_lookup(&owner);
    owner.edit(
        "interfaces/shelf-lookup.yaml",
        "        name: \"The returned book has the title asked for.\"\n",
        &format!(
            "        name: \"The returned book has the title asked for.\"\n      - key: {DROPPED}\n        \
             name: \"The shelf names the place of the book.\"\n"
        ),
    );
    fs::write(
        owner.file("interfaces/shelf-stock.yaml"),
        format!(
            "key: {STOCK}\nslug: shelf-stock\nname: \"Shelf stock\"\nbody: \"How many copies a \
             shelf holds.\"\nrequirements:\n  - key: 00000000-0000-4000-8000-0000000000c1\n    \
             statement:\n      form: ubiquitous\n      system: \"the shelf store\"\n      shall: \
             \"count the copies of each book.\"\n"
        ),
    )
    .unwrap();
    let mut document = codec::read(&tree(&owner.0)).unwrap();
    document.id = DocumentId::new(Key::parse(OTHER).unwrap());
    let (first, files) = publish_at(&codec::write(&document), None, EPOCH + 60);
    let mut states = vec![stored(&first, &files)];
    let mut document = codec::read(&files).unwrap();
    document
        .interfaces
        .retain(|interface| interface.slug.as_str() != "shelf-stock");
    let criteria = &mut document.interfaces[0].requirements[0].criteria;
    "The returned book has the title and the author asked for.".clone_into(&mut criteria[0].name);
    let mut added = criteria[0].clone();
    added.id = Some(CriterionId::new(Key::parse(ADDED).unwrap()));
    "The returned book is on the shelf.".clone_into(&mut added.name);
    criteria
        .retain(|criterion| criterion.id != Some(CriterionId::new(Key::parse(DROPPED).unwrap())));
    criteria.push(added);
    publish_labels(&mut document);
    let (second, files) = publish_at(&codec::write(&document), Some(&first), EPOCH + 120);
    states.push(stored(&second, &files));
    History {
        key: OTHER.to_owned(),
        states,
    }
}

/// The other document's first state, then a second in which "Shelf lookup" and "Shelf stock"
/// trade slugs, each keeping its key, and "Shelf stock" is renamed "Shelf stock count".
pub fn swapping() -> History {
    let first = shelves().states.swap_remove(0);
    let mut document = codec::read(&spec(&first.files)).unwrap();
    let slugs: Vec<Slug> = (document.interfaces.iter())
        .map(|interface| interface.slug.clone())
        .collect();
    for (interface, slug) in document.interfaces.iter_mut().zip(slugs.into_iter().rev()) {
        interface.slug = slug;
        if interface.name == "Shelf stock" {
            interface.name.push_str(" count");
        }
    }
    let previous = Lock::read(first.lock.to_string().as_bytes()).unwrap();
    let (second, files) = publish_at(&codec::write(&document), Some(&previous), EPOCH + 180);
    History {
        key: OTHER.to_owned(),
        states: vec![first, stored(&second, &files)],
    }
}

/// The other document's first state, then a second in which the lookup's criterion
/// [`DROPPED`] moves, keeping its key, to the Requirement of "Shelf stock".
pub fn moving() -> History {
    let first = shelves().states.swap_remove(0);
    let mut document = codec::read(&spec(&first.files)).unwrap();
    let dropped = Some(CriterionId::new(Key::parse(DROPPED).unwrap()));
    let criteria = &mut document.interfaces[0].requirements[0].criteria;
    let at = criteria
        .iter()
        .position(|criterion| criterion.id == dropped)
        .unwrap();
    let moved = criteria.remove(at);
    document.interfaces[1].requirements[0].criteria.push(moved);
    let previous = Lock::read(first.lock.to_string().as_bytes()).unwrap();
    let (second, files) = publish_at(&codec::write(&document), Some(&previous), EPOCH + 180);
    History {
        key: OTHER.to_owned(),
        states: vec![first, stored(&second, &files)],
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

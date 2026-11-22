//! `loft pull` of a head that lists another document and holds a copy of its Interface: the
//! state verifies with its pins, and merges three ways into a tree with a change of its own.

use loftspec::codec::{self, SpecPath, copy_of};
use loftspec::document::OtherDocument;
use loftspec::identity::Key;
use loftspec::ids::DocumentId;
use loftspec::publish::{self, Lock, Made, Publication, PublishedAt, ReadTree};
use loftspec::slug::Slug;
use software_plant::remote::{RemoteScenario, StoredState};

use crate::common::connected::{Connected, connected};
use crate::common::copies::{HANDLE, OTHER, OTHER_REMOTE};
use crate::common::others::{COPY, shelves};
use crate::common::remote::{spec, stored};
use crate::common::signed_in::document;
use crate::common::{EPOCH, tree};
use crate::interfaces::LOOKUP;
use crate::pull::{BODY, PRODUCT, calls, pull};

/// State 3 of the Reading List, as its remote publishes it after `previous`: connected,
/// listing the other document under [`HANDLE`] and holding a copy of its "Shelf lookup" at
/// publish sequence 2. Its manifest names no remote, so that the tree's own address, which the
/// earlier states do not name either, merges in unopposed.
fn listing_after(previous: &StoredState) -> StoredState {
    let mut document = codec::read(&spec(&previous.files)).unwrap();
    document.connected = true;
    let handle = Slug::parse(HANDLE).unwrap();
    document.documents.push(OtherDocument {
        handle: handle.clone(),
        key: DocumentId::new(Key::parse(OTHER).unwrap()),
        remote: OTHER_REMOTE.to_owned(),
    });
    let other = &shelves().states[1];
    let root = other.lock["stateRoot"].as_str().unwrap().parse().unwrap();
    let lookup = Key::parse(LOOKUP).unwrap();
    let copy = copy_of(&handle, &spec(&other.files), lookup, 2, root).unwrap();
    document.copies.push(copy);
    let files = codec::write(&document);
    let previous = Lock::read(previous.lock.to_string().as_bytes()).unwrap();
    let at = PublishedAt::from_unix_seconds(EPOCH + 3600).unwrap();
    let tree = ReadTree::read(&files).unwrap();
    let Publication::Published {
        lock,
        files: written,
        ..
    } = publish::publish(tree, None, Some(&previous), at, Made::AsRemote).unwrap()
    else {
        panic!("the remote publishes the change");
    };
    let mut files = files;
    files.extend(written);
    stored(&lock, &files)
}

/// A repository at state 2, connected to a remote whose head is [`listing_after`] state 2.
fn behind_a_listing_head() -> (Connected, StoredState) {
    let run = connected(2, 2, RemoteScenario::accepting);
    let head = listing_after(&run.history.states[1]);
    let mut states = run.history.states.clone();
    states.push(head.clone());
    let key = run.history.key.clone();
    let run = run.serving(RemoteScenario::accepting(&key).holding("Reading List", states));
    (run, head)
}

#[test]
fn a_head_with_documents_and_a_copy_verifies_and_merges_three_ways() {
    let (run, head) = behind_a_listing_head();
    run.scratch.edit(
        PRODUCT,
        BODY,
        "Keeps the books a reader means to read next.",
    );
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "pulled");
    assert_eq!(result["publishSequence"], 3);
    assert_eq!(calls(&run), ["/head", "/states/2", "/states/3"]);
    let files = tree(&run.scratch.0);
    let copy = SpecPath::new(COPY);
    assert_eq!(files[&copy], head.files[&format!("spec/{COPY}")].as_bytes());
    let product = String::from_utf8(files[&SpecPath::new(PRODUCT)].clone()).unwrap();
    assert!(product.contains("means to read next."), "ours is kept");
    let pulled = codec::read(&files).unwrap();
    assert_eq!(pulled.documents.len(), 1);
    assert_eq!(pulled.copies.len(), 1);
    assert_eq!(run.lock(), head.lock, "the lock is the head's");
    let check = run.scratch.loft(&["check"]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
}

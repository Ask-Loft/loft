//! The manifest's `documents`: the other connected documents a connected document links to,
//! each under a handle, with its document key and the address of its remote. Its entries are
//! written sorted by handle, refused outside a connected document, refused twice and refused
//! for the document itself, and enter no root.

use crate::common::fields::{assert_required, fields, scalar};
use crate::common::full::{full_document, other_document};
use crate::common::small_document;
use crate::common::text::{path, replaced, text};
use loftspec::codec::{self, Files, ReadError};
use loftspec::document::Document;
use loftspec::hash::hash_document;
use loftspec::slug::Slug;
use loftspec::yaml::Location;

const MANIFEST: &str = "loftspec.yaml";
const ENTRY: &[&str] = &["documents", "0"];

/// The full document, connected, listing the other documents given.
fn listing(others: &[(&str, u32)]) -> Document {
    let mut document = full_document();
    document.documents = others
        .iter()
        .map(|&(handle, n)| other_document(handle, n))
        .collect();
    document
}

fn files(others: &[(&str, u32)]) -> Files {
    codec::write(&listing(others))
}

/// The one error the files give, naming the manifest.
fn refused(files: &Files) -> ReadError {
    let errors = codec::read(files).expect_err("refused");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].sites[0].path, path(MANIFEST));
    errors.into_iter().next().unwrap()
}

/// The line of the manifest's text that `needle` starts in, one-based.
fn line_of(files: &Files, needle: &str) -> usize {
    let manifest = text(files, MANIFEST);
    let at = manifest.find(needle).expect(needle);
    manifest[..at].matches('\n').count() + 1
}

#[test]
fn an_entry_of_documents_holds_handle_key_and_remote_in_order_and_requires_each() {
    let published = ["handle", "key", "remote"];
    assert_eq!(
        fields(&files(&[("format", 80)]), MANIFEST, ENTRY),
        published
    );
    for field in published {
        assert_required(&files(&[("format", 80)]), MANIFEST, ENTRY, field);
    }
}

#[test]
fn documents_is_written_sorted_by_handle_in_byte_order() {
    let files = files(&[("store", 81), ("format-v2", 82), ("format", 83)]);
    let handles: Vec<String> = (0..3)
        .map(|n| scalar(&files, MANIFEST, &["documents", &n.to_string(), "handle"]))
        .collect();
    assert_eq!(handles, ["format", "format-v2", "store"]);
}

#[test]
fn an_entry_records_the_other_documents_key_and_the_address_of_its_remote() {
    let document = listing(&[("format", 80)]);
    let files = codec::write(&document);
    let key = scalar(&files, MANIFEST, &["documents", "0", "key"]);
    assert_eq!(key, document.documents[0].key.to_string());
    let remote = scalar(&files, MANIFEST, &["documents", "0", "remote"]);
    assert_eq!(remote, "https://remote.example/documents/format");
    assert_eq!(codec::read(&files).unwrap().documents, document.documents);
}

#[test]
fn a_handle_that_is_not_a_slug_is_refused() {
    let files = files(&[("format", 80)]);
    let wrong = replaced(&files, MANIFEST, "handle: format", "handle: \"Format One\"");
    refused(&wrong);
    assert!(Slug::parse("Format One").is_err());
}

#[test]
fn a_manifest_without_documents_lists_no_other_document() {
    let files = codec::write(&small_document());
    assert!(!text(&files, MANIFEST).contains("documents"));
    assert_eq!(codec::read(&files).unwrap().documents, []);
}

#[test]
fn documents_without_connected_is_refused_naming_the_file_line_and_column() {
    let files = files(&[("format", 80)]);
    let wrong = replaced(&files, MANIFEST, "connected: true\n", "");
    let wrong = replaced(
        &wrong,
        MANIFEST,
        "remote: \"https://remote.example/documents/one\"\n",
        "",
    );
    let error = refused(&wrong);
    assert!(
        error
            .to_string()
            .contains("only a connected document links to other documents"),
        "{error}"
    );
    let Some(Location::At(at)) = error.sites[0].location else {
        panic!("{error:?}");
    };
    assert_eq!(at.line, line_of(&wrong, "handle: format"));
}

#[test]
fn a_connected_documents_list_raises_no_error() {
    assert!(codec::read(&files(&[("format", 80), ("store", 81)])).is_ok());
}

#[test]
fn two_entries_with_one_handle_are_refused_naming_the_handle_at_the_later_entry() {
    let files = files(&[("format", 80), ("store", 81)]);
    let wrong = replaced(&files, MANIFEST, "handle: store", "handle: format");
    let error = refused(&wrong);
    assert!(error.to_string().contains("handle format"), "{error}");
    let Some(Location::At(at)) = error.sites[0].location else {
        panic!("{error:?}");
    };
    let manifest = text(&wrong, MANIFEST);
    let (second, _) = manifest.match_indices("handle: format").nth(1).unwrap();
    let line = manifest[..second].matches('\n').count() + 1;
    assert_eq!(at.line, line);
}

#[test]
fn two_entries_with_one_key_are_refused_naming_the_key() {
    let document = listing(&[("format", 80), ("store", 81)]);
    let files = codec::write(&document);
    let (first, second) = (document.documents[0].key, document.documents[1].key);
    let wrong = replaced(&files, MANIFEST, &second.to_string(), &first.to_string());
    let error = refused(&wrong);
    assert!(error.to_string().contains(&first.to_string()), "{error}");
}

#[test]
fn an_entry_with_the_documents_own_key_is_refused_naming_the_key() {
    let document = listing(&[("format", 80)]);
    let files = codec::write(&document);
    let own = document.id.to_string();
    let other = document.documents[0].key.to_string();
    let error = refused(&replaced(
        &files,
        MANIFEST,
        &format!("key: {other}"),
        &format!("key: {own}"),
    ));
    let message = error.to_string();
    assert!(message.contains(&own), "{message}");
    assert!(message.contains("does not link to itself"), "{message}");
}

#[test]
fn documents_enters_no_root() {
    let roots = |others: &[(&str, u32)]| hash_document(&listing(others)).unwrap().roots;
    let listed = roots(&[("format", 80)]);
    assert_eq!(roots(&[]), listed);
    let mut moved = listing(&[("format", 80)]);
    moved.documents[0].remote = "https://elsewhere.example/documents/format".to_owned();
    assert_eq!(hash_document(&moved).unwrap().roots, listed);
    assert_eq!(roots(&[("renamed", 80)]), listed);
}

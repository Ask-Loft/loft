//! The copies of other documents' Interfaces: written under `spec/documents/<handle>/`, read
//! apart from the document's own nodes, refused once edited, refused under a handle the
//! manifest does not list, and kept out of labels, lint, formatting and every root.

use crate::common::copies::{COPY, HANDLE, OWNER, copy_at, owner, requiring, state_root};
use crate::common::fields::{assert_required, fields, scalar};
use crate::common::full::other_document;
use crate::common::interfaces::{INTERFACE, with_interface};
use crate::common::lint::findings;
use crate::common::text::{indented, path, replaced, text};
use crate::common::{key, small_document};
use loftspec::codec::{self, Files, Problem, ReadError};
use loftspec::fmt::format;
use loftspec::hash::{ContentHash, hash_document};
use loftspec::identity::Key;
use loftspec::publish::allocate;

fn files() -> Files {
    codec::write(&requiring())
}

fn errors(files: &Files) -> Vec<ReadError> {
    codec::read(files).expect_err("refused")
}

/// The one error the files give.
fn refused(files: &Files) -> ReadError {
    let errors = errors(files);
    assert_eq!(errors.len(), 1, "{errors:?}");
    errors.into_iter().next().unwrap()
}

#[test]
fn a_copy_is_written_under_its_handle_with_the_slug_of_its_interface() {
    let files = files();
    assert!(files.contains_key(&path(COPY)), "{:?}", files.keys());
    assert_eq!(owner().interfaces[0].slug.as_str(), "document-lookup");
    let read = codec::read(&files).unwrap();
    let expected = requiring();
    assert_eq!(read.copies, expected.copies);
    assert_eq!(read.documents, expected.documents);
}

#[test]
fn a_copy_holds_its_five_fields_in_order_and_requires_each() {
    let published = [
        "publishSequence",
        "stateRoot",
        "interfaceHash",
        "labels",
        "interface",
    ];
    assert_eq!(fields(&files(), COPY, &[]), published);
    for field in published {
        assert_required(&files(), COPY, &[], field);
    }
}

#[test]
fn a_copy_names_the_published_state_it_was_taken_from() {
    let files = files();
    assert_eq!(scalar(&files, COPY, &["publishSequence"]), "3");
    assert_eq!(
        scalar(&files, COPY, &["stateRoot"]),
        state_root(3).to_string()
    );
}

#[test]
fn a_copys_interface_hash_is_the_interfaces_content_hash() {
    let hash = owner().interfaces[0].content_hash().to_string();
    assert_eq!(scalar(&files(), COPY, &["interfaceHash"]), hash);
}

#[test]
fn a_copys_labels_are_its_nodes_labels_sorted_by_key() {
    let owner = owner();
    let copy = &requiring().copies[0];
    let expected: Vec<(Key, String)> = [INTERFACE, INTERFACE + 1, INTERFACE + 2]
        .map(|n| {
            (
                key(n),
                owner.bindings.label_of_key(key(n)).unwrap().to_string(),
            )
        })
        .to_vec();
    let written: Vec<(Key, String)> = (0..3)
        .map(|n| {
            let at = |field| scalar(&files(), COPY, &["labels", &n.to_string(), field]);
            (Key::parse(&at("key")).unwrap(), at("label"))
        })
        .collect();
    assert_eq!(written, expected);
    assert_eq!(copy.labels.len(), 3);
    assert_eq!(expected[0].1, "IF-1");
}

#[test]
fn a_copy_holds_the_interface_record_as_its_document_holds_it() {
    let source = text(&codec::write(&owner()), "interfaces/document-lookup.yaml");
    let copy = text(&files(), COPY);
    assert!(
        copy.contains(&format!("interface:\n{}", indented(&source, "  "))),
        "{copy}"
    );
}

#[test]
fn a_copy_file_that_holds_more_than_one_interface_is_refused() {
    let files = files();
    let copy = text(&files, COPY);
    let (head, record) = copy.split_once("interface:\n").unwrap();
    let twice = format!(
        "{head}interface:\n  - {}  - {}",
        record.trim_start(),
        record.trim_start()
    );
    let error = refused(&replaced(&files, COPY, &copy, &twice));
    assert_eq!(error.sites[0].path, path(COPY));
}

#[test]
fn every_directory_under_documents_is_named_by_a_handle_of_documents() {
    let mut document = requiring();
    document.documents[0].handle = crate::common::slug("store");
    let error = refused(&codec::write(&document));
    assert_eq!(error.sites[0].path, path("documents/format"));
    assert!(error.to_string().contains("handle format"), "{error}");
}

#[test]
fn a_copy_in_a_document_that_is_not_connected_is_refused_naming_the_handle() {
    let mut document = requiring();
    document.connected = false;
    document.remote = None;
    document.documents.clear();
    let error = refused(&codec::write(&document));
    assert!(error.to_string().contains(HANDLE), "{error}");
    assert!(error.to_string().contains("connected"), "{error}");
}

#[test]
fn a_copys_nodes_enter_neither_the_content_root_nor_the_binding_root() {
    let mut without = requiring();
    without.copies.clear();
    let (with, without) = (
        hash_document(&requiring()).unwrap(),
        hash_document(&without).unwrap(),
    );
    assert_eq!(with.roots.content, without.roots.content);
    assert_eq!(with.roots.bindings, without.roots.bindings);
    assert!(!with.nodes.keys().any(|id| id.key() == key(INTERFACE)));
}

#[test]
fn a_publish_labels_no_node_of_a_copy_and_binds_none() {
    let mut document = requiring();
    let allocated = allocate(&mut document);
    let copied = document.copies[0].keys();
    assert!(
        allocated
            .iter()
            .all(|given| !copied.contains(&given.element.key()))
    );
    assert!(
        copied
            .iter()
            .all(|key| document.bindings.get(*key).is_none())
    );
}

#[test]
fn the_linter_reports_no_finding_on_a_node_of_a_copy() {
    let mut document = requiring();
    let promiseless = owner_without_requirements();
    document.copies = vec![copy_at(&promiseless, HANDLE, 3)];
    let copied = document.copies[0].keys();
    let named: Vec<Key> = (findings(&document).into_iter())
        .flat_map(|found| found.subjects)
        .map(|subject| subject.key)
        .collect();
    assert!(named.iter().all(|key| !copied.contains(key)), "{named:?}");
}

/// The other document with its Interface holding no Requirement, which `V2-012` would report.
fn owner_without_requirements() -> loftspec::document::Document {
    let mut document = with_interface();
    document.id = loftspec::ids::DocumentId::new(key(OWNER));
    document.interfaces[0].requirements.clear();
    allocate(&mut document);
    document
}

#[test]
fn the_formatter_leaves_every_copy_file_as_it_is() {
    let files = files();
    let formatted = format(&files).unwrap();
    assert!(
        !formatted.changed.contains_key(&path(COPY)),
        "{:?}",
        formatted.changed.keys()
    );
    let reordered = replaced(&files, COPY, "publishSequence: 3\n", "");
    let reordered = replaced(
        &reordered,
        COPY,
        "labels:\n",
        "publishSequence: 3\nlabels:\n",
    );
    let formatted = format(&reordered).unwrap();
    assert!(
        !formatted.changed.contains_key(&path(COPY)),
        "a copy out of layout stays as it is"
    );
}

#[test]
fn a_key_of_a_copy_that_an_element_of_the_document_carries_is_refused_naming_both_files() {
    let mut document = with_interface();
    let requiring = requiring();
    document.connected = true;
    document.remote = requiring.remote;
    document.documents = requiring.documents;
    document.copies = requiring.copies;
    let found = errors(&codec::write(&document));
    let named = found
        .iter()
        .find(|error| error.problem == Problem::DuplicateKey(key(INTERFACE)))
        .unwrap_or_else(|| panic!("{found:?}"));
    let paths: Vec<_> = named.sites.iter().map(|site| site.path.clone()).collect();
    assert_eq!(paths, [path("interfaces/document-lookup.yaml"), path(COPY)]);
}

#[test]
fn a_key_carried_by_nodes_of_two_copies_is_refused_naming_both_files() {
    let mut document = requiring();
    document
        .documents
        .push(other_document("format-again", OWNER + 1));
    document.copies.push(copy_at(&owner(), "format-again", 3));
    let found = errors(&codec::write(&document));
    let named = found
        .iter()
        .find(|error| error.problem == Problem::DuplicateKey(key(INTERFACE)))
        .unwrap_or_else(|| panic!("{found:?}"));
    let paths: Vec<_> = named.sites.iter().map(|site| site.path.clone()).collect();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&path(COPY)), "{paths:?}");
    assert!(paths.contains(&path("documents/format-again/document-lookup.yaml")));
}

#[test]
fn an_edit_to_a_node_of_a_copy_is_refused_naming_the_copys_file() {
    let files = files();
    let edited = replaced(
        &files,
        COPY,
        "The answer names the document.",
        "The answer names it.",
    );
    let error = refused(&edited);
    assert_eq!(error.sites[0].path, path(COPY));
    assert!(
        error
            .to_string()
            .contains("only `loft interface` writes a copy"),
        "{error}"
    );
}

#[test]
fn a_copy_as_it_was_written_raises_no_error() {
    assert!(codec::read(&files()).is_ok());
    assert!(codec::read(&codec::write(&small_document())).is_ok());
}

//! Converting a v1 export under a document key the caller names: an export with no element, as
//! a document's first published state can be, becomes an empty document under that key.

use crate::common::v1::{export, full_export, key, read, uuid};
use loftspec::codec;
use loftspec::convert::v1::{ConvertError, convert, convert_keyed};
use loftspec::ids::{AuthoredStateId, DocumentId};
use serde_json::{Value, json};

/// An export with no node, registry entry, link or decision.
fn empty_export() -> Value {
    let mut value = export(&[], &[]);
    value["vocabularyEntries"] = json!([]);
    value["technicalConstraintCategories"] = json!([]);
    value["realizationLinks"] = json!([]);
    value
}

fn named() -> DocumentId {
    DocumentId::new(key(50))
}

#[test]
fn an_empty_export_converts_to_an_empty_document_under_the_named_key() {
    let document = convert_keyed(&read(&empty_export()), named())
        .unwrap()
        .document;
    assert_eq!(document.id, named());
    assert!(document.products.is_empty() && document.components.is_empty());
    assert!(document.decisions.is_empty() && document.tags.is_empty());
    assert!(document.categories.is_empty() && document.bindings.is_empty());
    assert_eq!(codec::read(&codec::write(&document)).unwrap(), document);
}

#[test]
fn the_empty_document_derives_from_the_exports_state() {
    let document = convert_keyed(&read(&empty_export()), named())
        .unwrap()
        .document;
    let derived = document.derived_from.unwrap();
    assert_eq!(derived.authored_state_id, AuthoredStateId::new(key(99)));
    assert_eq!(derived.publish_sequence, 7);
}

#[test]
fn an_export_carrying_another_document_than_the_named_key_is_refused_naming_both() {
    let error = convert_keyed(&read(&full_export()), named()).unwrap_err();
    assert!(
        matches!(error, ConvertError::OtherDocument { .. }),
        "{error}"
    );
    let message = error.to_string();
    assert!(
        message.contains(&uuid(0)) && message.contains(&uuid(50)),
        "{message}"
    );
}

#[test]
fn an_export_carrying_the_named_key_converts_as_it_does_without_one() {
    let export = read(&full_export());
    let keyed = convert_keyed(&export, DocumentId::new(key(0))).unwrap();
    assert_eq!(keyed, convert(&export).unwrap());
}

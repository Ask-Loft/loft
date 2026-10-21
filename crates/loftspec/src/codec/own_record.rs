//! The own record view of each keyed record and its digest (Own record views): the record as the
//! writer writes it in canonical layout, without its key and without the records it holds, for
//! tools that store or compare records one at a time. No root holds it.

use std::collections::BTreeMap;

use serde_json::Value;

use super::canon::canonicalize;
use super::record::Rec;
use super::records::own_view;
use super::schema::Kind;
use super::unresolve::unresolve;
use super::unresolve_one::written_record;
use crate::canonical::{Digest, Domain, digest};
use crate::document::Document;
use crate::identity::Key;
use crate::kind::NodeKind;

/// What an own record is the record of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordKind {
    Node(NodeKind),
    Decision,
    /// An entry of the tag registry.
    Tag,
    /// An entry of the category registry.
    Category,
    /// An entry of the term list.
    Term,
    /// The binding of an element, keyed by the element's key.
    Binding,
}

impl RecordKind {
    /// The kind of a record that has an own record view; none for a part of a record, such as a
    /// decision's option or a Requirement's statement, and for the manifest and the counters.
    const fn of(kind: Kind) -> Option<Self> {
        Some(match kind {
            Kind::Decision => Self::Decision,
            Kind::TagEntry => Self::Tag,
            Kind::CategoryEntry => Self::Category,
            Kind::TermEntry => Self::Term,
            Kind::Binding => Self::Binding,
            _ => match kind.node_kind() {
                Some(node) => Self::Node(node),
                None => return None,
            },
        })
    }
}

/// One record's own record view and its digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnRecord {
    pub kind: RecordKind,
    /// The record's fields as the writer writes them, by field name, without its key and the
    /// records it holds.
    pub view: Value,
    pub digest: Digest,
}

/// The own record of every keyed node, decision, registry entry and binding of a document, by
/// kind and key. A binding is keyed by the key of the element it binds. A record's conflicts are
/// no part of its view. The document need not index: no view follows a reference, and where two
/// records of one kind share a key, the first the writer writes is kept.
#[must_use]
pub fn own_records(document: &Document) -> BTreeMap<(RecordKind, Key), OwnRecord> {
    let mut tree = unresolve(document);
    canonicalize(&mut tree);
    let mut records = BTreeMap::new();
    for (_, content) in tree.files.values() {
        for record in content.records() {
            collect(record, &mut records);
        }
    }
    records
}

/// The own record of the record of kind `kind` keyed `key`, built alone: the entry
/// [`own_records`] gives it in a document that carries each key once, without building the own
/// record view of any other record. None where the document holds no record of that kind under
/// that key. A binding is found by the key of the element it binds.
#[must_use]
pub fn own_record(document: &Document, kind: RecordKind, key: Key) -> Option<OwnRecord> {
    let record = written_record(document, kind, key)?;
    let view = own_view(&record);
    let digest = own_record_digest(&view);
    Some(OwnRecord { kind, view, digest })
}

/// The digest of an own record view, under `loftspec/own-record/1`.
#[must_use]
pub fn own_record_digest(view: &Value) -> Digest {
    digest(Domain::OwnRecord, view)
}

/// Adds the own record of `record` and of every record beneath it.
fn collect(record: &Rec, records: &mut BTreeMap<(RecordKind, Key), OwnRecord>) {
    if let Some(kind) = RecordKind::of(record.kind)
        && let Some(key) = record.key()
    {
        records.entry((kind, key)).or_insert_with(|| {
            let view = own_view(record);
            let digest = own_record_digest(&view);
            OwnRecord { kind, view, digest }
        });
    }
    for child in record.children() {
        collect(child, records);
    }
}

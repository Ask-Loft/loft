//! Each node's and each decision's record as the text form writes it, as JSON, for the comparer
//! (Comparing two versions). A node's record is its own: the records of the nodes beneath it,
//! and a Feature Group's list of its Features' keys, are left out.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::SpecPath;
use super::canon::canonicalize;
use super::files::keyed_records;
use super::record::{Rec, Val};
use super::schema::{Field, Item, Kind, Scalar, field};
use super::unresolve::unresolve;
use crate::document::Document;
use crate::ids::{DecisionId, NodeId};

/// The records of a document's nodes and decisions, by ID, and the file of each node without a
/// key.
#[derive(Debug, Default)]
pub(crate) struct Records {
    pub nodes: BTreeMap<NodeId, Value>,
    pub decisions: BTreeMap<DecisionId, Value>,
    pub unkeyed: Vec<SpecPath>,
}

/// The records the writer writes for `document`, in canonical layout.
pub(crate) fn records(document: &Document) -> Records {
    let mut tree = unresolve(document);
    canonicalize(&mut tree);
    let mut records = Records::default();
    for (path, (_, content)) in &tree.files {
        for record in keyed_records(content.records()) {
            records.add(path, record);
        }
    }
    records
}

impl Records {
    fn add(&mut self, path: &SpecPath, record: &Rec) {
        if record.kind == Kind::Decision {
            let key = record.key().expect("the writer writes a decision's key");
            self.decisions
                .insert(DecisionId::new(key), json(record, false));
        } else if let Some(kind) = record.kind.node_kind() {
            match record.key() {
                Some(key) => {
                    self.nodes
                        .insert(NodeId::new(kind, key), json(record, true));
                }
                None => self.unkeyed.push(path.clone()),
            }
        }
    }
}

/// A record as JSON, its fields as the writer writes them, an empty list left out; with `own`,
/// without the fields that hold the nodes beneath it.
fn json(record: &Rec, own: bool) -> Value {
    let mut fields = Map::new();
    for (name, value) in &record.fields {
        let (_, field) = record.kind.field(name).expect("a field the schema defines");
        let empty = matches!(value, Val::List(items) if items.is_empty());
        if empty || (own && holds_nodes(field)) {
            continue;
        }
        fields.insert((*name).to_owned(), value_json(value, field.shape.item()));
    }
    Value::Object(fields)
}

/// Whether a field holds the nodes beneath its record, or the list of their keys.
fn holds_nodes(field: &Field) -> bool {
    let records_nodes =
        matches!(field.shape.item(), Item::Record(kind) if kind.node_kind().is_some());
    records_nodes || field.name == field::FEATURES
}

fn value_json(value: &Val, item: Item) -> Value {
    match value {
        Val::Scalar { text, .. } => match item {
            Item::Scalar(Scalar::Flag) => Value::Bool(true),
            Item::Scalar(Scalar::Integer) => text
                .parse::<u64>()
                .map_or_else(|_| Value::String(text.clone()), Value::from),
            _ => Value::String(text.clone()),
        },
        Val::Record(record) => json(record, false),
        Val::List(items) => items.iter().map(|entry| value_json(entry, item)).collect(),
        Val::Null { .. } => Value::Null,
        Val::Ext { value, .. } => crate::ext::to_json(value),
    }
}

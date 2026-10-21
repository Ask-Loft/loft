//! A typed merge conflict in the model (F-34): which record holds it, the field it names,
//! and the value each side gave that field. The validator reports every conflict until it is
//! removed (RQ-126), and no hash of the document reads it. Each conflict is named by an
//! identifier derived from it alone, which no file holds.

mod references;
mod settle;
mod structure;

pub use settle::{SettleError, Settlement, settle};

use crate::canonical::{Digest, Domain, digest};
use crate::codec::RecordKind;
use crate::codec::{LABELS, MANIFEST, SpecPath};
use crate::ext::Ext;
use serde_json::{Value, json};
use std::fmt;

use crate::identity::Key;
use crate::ids::{CategoryId, ConsequenceId, DriverId, ElementId, OptionId, TagId, TermId};

/// The record that holds a conflict: a node, a decision or a part of one with its own key, a
/// registry entry, or the manifest or the bindings record, which carry no key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConflictOwner {
    Element(ElementId),
    Option(OptionId),
    Driver(DriverId),
    Consequence(ConsequenceId),
    Tag(TagId),
    Category(CategoryId),
    Term(TermId),
    Manifest,
    Labels,
}

impl ConflictOwner {
    /// The key of the record that holds the conflict; none for the manifest or the bindings.
    #[must_use]
    pub const fn key(self) -> Option<Key> {
        Some(match self {
            Self::Element(element) => element.key(),
            Self::Option(option) => option.key(),
            Self::Driver(driver) => driver.key(),
            Self::Consequence(consequence) => consequence.key(),
            Self::Tag(tag) => tag.key(),
            Self::Category(category) => category.key(),
            Self::Term(term) => term.key(),
            Self::Manifest | Self::Labels => return None,
        })
    }
}

impl ConflictOwner {
    /// The own record of the conflicts' owner, by its kind and key, as `codec::own_records` keys
    /// it: a node's, a decision's or a registry entry's; none for a decision's option, driver or
    /// consequence, whose decision's own record holds it, and for the manifest and the bindings.
    #[must_use]
    pub const fn own_record(self) -> Option<(RecordKind, Key)> {
        Some(match self {
            Self::Element(ElementId::Node(node)) => (RecordKind::Node(node.kind()), node.key()),
            Self::Element(ElementId::Decision(decision)) => (RecordKind::Decision, decision.key()),
            Self::Tag(tag) => (RecordKind::Tag, tag.key()),
            Self::Category(category) => (RecordKind::Category, category.key()),
            Self::Term(term) => (RecordKind::Term, term.key()),
            Self::Option(_)
            | Self::Driver(_)
            | Self::Consequence(_)
            | Self::Manifest
            | Self::Labels => {
                return None;
            }
        })
    }

    /// The holder a conflict's identifier names: the key of the record whose `conflicts` list
    /// holds it, `spec/loftspec.yaml` for the manifest and `spec/labels.yaml` for the bindings.
    #[must_use]
    pub fn holder(self) -> String {
        match self {
            Self::Manifest => SpecPath::new(MANIFEST).to_string(),
            Self::Labels => SpecPath::new(LABELS).to_string(),
            other => other
                .key()
                .expect("every other holder carries a key")
                .to_string(),
        }
    }
}

impl fmt::Display for ConflictOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(element) => write!(formatter, "{element}"),
            Self::Option(option) => write!(formatter, "option {option}"),
            Self::Driver(driver) => write!(formatter, "driver {driver}"),
            Self::Consequence(consequence) => write!(formatter, "consequence {consequence}"),
            Self::Tag(tag) => write!(formatter, "tag entry {tag}"),
            Self::Category(category) => write!(formatter, "category entry {category}"),
            Self::Term(term) => write!(formatter, "term entry {term}"),
            Self::Manifest => formatter.write_str("the manifest"),
            Self::Labels => formatter.write_str("the bindings"),
        }
    }
}

/// The field a conflict names, from the record that holds it: a field name, a path dotted
/// through the unkeyed records inside it (`statement.shall`), `position`, `parent` or `record`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldPath(String);

impl FieldPath {
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One side's value of a conflicting field, as the text form holds it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConflictValue {
    /// The side deleted the record (`null`).
    Deleted,
    Scalar(String),
    List(Vec<Self>),
    /// An unkeyed record: its fields in the Format's order.
    Record(Vec<(String, Self)>),
    /// A node's extension data.
    Ext(Ext),
}

/// A conflict: the field two sides changed to different values, and each side's value; a side
/// on which the field is absent has none (AC-859).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Conflict {
    pub owner: ConflictOwner,
    pub field: FieldPath,
    pub base: Option<ConflictValue>,
    pub ours: Option<ConflictValue>,
    pub theirs: Option<ConflictValue>,
}

/// A conflict record's identifier: SHA-256 under `loftspec/conflict/1` over the canonical JSON
/// of the array of its holder, its field, `base`, `ours` and `theirs`, written as 64 lowercase
/// hexadecimal digits. It is derived from the record and its holder alone and never stored, so
/// two merges that write one conflict name it alike, and resolving another leaves it as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConflictId(Digest);

impl ConflictId {
    #[must_use]
    pub const fn digest(self) -> Digest {
        self.0
    }
}

impl fmt::Display for ConflictId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Conflict {
    /// The conflict's identifier. Each side enters the view as the conflict record writes it: a
    /// scalar as its text, a number or a flag included, a sequence as an array, a mapping as an
    /// object, extension data as the JSON it holds, and a side that deleted the record or lacks
    /// the field as `null`.
    #[must_use]
    pub fn id(&self) -> ConflictId {
        ConflictId(digest(Domain::Conflict, &self.view()))
    }

    /// The view the identifier digests: the array of the holder, the field, `base`, `ours` and
    /// `theirs`.
    #[must_use]
    pub fn view(&self) -> Value {
        let side = |side: &Option<ConflictValue>| side.as_ref().map_or(Value::Null, written);
        json!([
            self.owner.holder(),
            self.field.as_str(),
            side(&self.base),
            side(&self.ours),
            side(&self.theirs),
        ])
    }
}

/// A side as JSON, as the conflict record writes it.
fn written(side: &ConflictValue) -> Value {
    match side {
        ConflictValue::Deleted => Value::Null,
        ConflictValue::Scalar(text) => Value::String(text.clone()),
        ConflictValue::List(items) => items.iter().map(written).collect(),
        ConflictValue::Record(fields) => fields
            .iter()
            .map(|(name, value)| (name.clone(), written(value)))
            .collect(),
        ConflictValue::Ext(ext) => crate::ext::to_json(ext),
    }
}

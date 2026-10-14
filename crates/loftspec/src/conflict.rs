//! A typed merge conflict in the model (F-34): which record holds it, the field it names,
//! and the value each side gave that field. The validator reports every conflict until it is
//! removed (RQ-126), and no hash reads it.

use std::fmt;

use crate::identity::Key;
use crate::ids::{CategoryId, ConsequenceId, DriverId, ElementId, OptionId, TagId};

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
            Self::Manifest | Self::Labels => return None,
        })
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

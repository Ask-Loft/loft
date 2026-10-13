//! The record schema of the text form (F-49): each record kind's fields in the Format's
//! order, which are required, and what each field holds. One table drives reading, formatting
//! and writing, so the three cannot disagree on a field's name, place or style.

/// Every kind of record a file of the text form holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Manifest,
    DerivedFrom,
    Labels,
    Counters,
    Binding,
    TagEntry,
    CategoryEntry,
    Product,
    FeatureGroup,
    Feature,
    Requirement,
    Statement,
    Criterion,
    RootComponent,
    Component,
    Constraint,
    ConstraintBinding,
    QualityAttribute,
    QualityTarget,
    Band,
    Section,
    Decision,
    State,
    Driver,
    Option,
    Assessment,
    Consequence,
    Subject,
    FormerLabel,
}

/// What a scalar field holds, which fixes how it is checked and written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    /// A lowercase hyphenated UUID (RQ-60).
    Key,
    Label,
    Slug,
    Tag,
    Category,
    Digest,
    /// A positive integer, written plain (AC-829).
    Integer,
    /// One of a closed set of values, in `snake_case` (AC-793).
    Enum(&'static [&'static str]),
    /// An identifier such as a format or algorithm name, written plain where it can be.
    Identifier,
    /// Free text: double-quoted or a literal block, never plain (AC-831).
    Text,
}

/// What a field holds: one item, or a list of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    One(Item),
    List(Item),
}

/// What a field, or each entry of a list field, holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Scalar(Scalar),
    Record(Kind),
    /// A node's key, or a former label as `{formerLabel: <label>}` (AC-809).
    NodeRef,
}

/// The order the canonical layout gives a list field's entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// As the author wrote them, such as the Features a group lists (RQ-28).
    Authored,
    /// By key: edge targets (AC-807) and label bindings.
    ByKey,
    /// By role, then by target (AC-808).
    ByRoleThenTarget,
}

/// How the formatter supplies a required field a record lacks (RQ-59, RQ-213, RQ-218).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supply {
    /// A fresh version 7 key (RQ-3, RQ-218).
    MintKey,
    /// The name of the record's file or directory (AC-818).
    SlugFromPath,
    /// The directory the record's listed Features sit in, or else the slug its name derives,
    /// unique among its siblings (AC-819, AC-820).
    SlugFromName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub required: bool,
    pub shape: Shape,
    pub order: Order,
    /// How the formatter supplies the field when it is missing; none for a field it never
    /// supplies.
    pub supply: Option<Supply>,
}

pub use super::field;

mod tables;

use crate::kind::NodeKind;

impl Kind {
    /// Every kind of record.
    pub const ALL: [Self; 29] = [
        Self::Manifest,
        Self::DerivedFrom,
        Self::Labels,
        Self::Counters,
        Self::Binding,
        Self::TagEntry,
        Self::CategoryEntry,
        Self::Product,
        Self::FeatureGroup,
        Self::Feature,
        Self::Requirement,
        Self::Statement,
        Self::Criterion,
        Self::RootComponent,
        Self::Component,
        Self::Constraint,
        Self::ConstraintBinding,
        Self::QualityAttribute,
        Self::QualityTarget,
        Self::Band,
        Self::Section,
        Self::Decision,
        Self::State,
        Self::Driver,
        Self::Option,
        Self::Assessment,
        Self::Consequence,
        Self::Subject,
        Self::FormerLabel,
    ];

    /// The kind's fields in the Format's order.
    #[must_use]
    pub const fn fields(self) -> &'static [Field] {
        tables::fields(self)
    }

    /// A field of the kind, by name, with its place in the Format's order.
    #[must_use]
    pub fn field(self, name: &str) -> Option<(usize, &'static Field)> {
        self.fields()
            .iter()
            .enumerate()
            .find(|(_, field)| field.name == name)
    }

    /// Whether the record carries its own key, which the formatter mints when it is missing
    /// (RQ-3, RQ-218).
    #[must_use]
    pub const fn is_keyed(self) -> bool {
        let fields = self.fields();
        let mut index = 0;
        while index < fields.len() {
            if matches!(fields[index].supply, Some(Supply::MintKey)) {
                return true;
            }
            index += 1;
        }
        false
    }

    /// Whether a record of the kind may hold a `conflicts` list: every keyed record, the
    /// manifest and the bindings record (RQ-172).
    #[must_use]
    pub const fn hosts_conflicts(self) -> bool {
        self.is_keyed() || matches!(self, Self::Manifest | Self::Labels)
    }

    /// The node kind a record of this kind is; none for a record that is not a node.
    #[must_use]
    pub const fn node_kind(self) -> Option<NodeKind> {
        Some(match self {
            Self::Product => NodeKind::Product,
            Self::FeatureGroup => NodeKind::FeatureGroup,
            Self::Feature => NodeKind::Feature,
            Self::Requirement => NodeKind::Requirement,
            Self::Criterion => NodeKind::Criterion,
            Self::RootComponent | Self::Component => NodeKind::Component,
            Self::Constraint => NodeKind::Constraint,
            Self::QualityAttribute => NodeKind::QualityAttribute,
            Self::Section => NodeKind::Section,
            _ => return None,
        })
    }
}

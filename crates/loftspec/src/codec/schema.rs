//! The record schema of the text form (D-11 F-49): each record kind's fields in the Format's
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

const fn spec(name: &'static str, required: bool, shape: Shape) -> Field {
    Field {
        name,
        required,
        shape,
        order: Order::Authored,
        supply: None,
    }
}

const fn req(name: &'static str, scalar: Scalar) -> Field {
    spec(name, true, Shape::One(Item::Scalar(scalar)))
}

const fn opt(name: &'static str, scalar: Scalar) -> Field {
    spec(name, false, Shape::One(Item::Scalar(scalar)))
}

const fn record(name: &'static str, required: bool, kind: Kind) -> Field {
    spec(name, required, Shape::One(Item::Record(kind)))
}

const fn node_ref(name: &'static str, required: bool) -> Field {
    spec(name, required, Shape::One(Item::NodeRef))
}

const fn records(name: &'static str, kind: Kind) -> Field {
    spec(name, false, Shape::List(Item::Record(kind)))
}

const fn scalars(name: &'static str, scalar: Scalar) -> Field {
    spec(name, false, Shape::List(Item::Scalar(scalar)))
}

const fn ordered(field: Field, order: Order) -> Field {
    Field { order, ..field }
}

const fn supplied(field: Field, supply: Supply) -> Field {
    Field {
        supply: Some(supply),
        ..field
    }
}

/// A list of keys the writer sorts: an edge field (RQ-168).
const fn edges(name: &'static str) -> Field {
    ordered(scalars(name, Scalar::Key), Order::ByKey)
}

pub use super::field;

use crate::attributes::{BandKind, ConservationKind, QaKind, QualityAggregation, VerificationFate};
use crate::decision::{ConsequenceKind, DriverKind, Rating, StateKind};
use crate::document::SubjectRole;
use crate::identity::LabelPrefix;
use crate::kind::NodeKind;
use crate::statement::Form;
use Kind as K;
use Scalar as S;

/// The key of a keyed record, which the formatter mints when it is missing.
const KEY: Field = supplied(req(field::KEY, S::Key), Supply::MintKey);
const SLUG: Field = supplied(req(field::SLUG, S::Slug), Supply::SlugFromPath);
/// A Feature Group's slug: it has no file of its own to take a name from.
const GROUP_SLUG: Field = supplied(req(field::SLUG, S::Slug), Supply::SlugFromName);
const NAME: Field = req(field::NAME, S::Text);
const BODY: Field = opt(field::BODY, S::Text);
const TAGS: Field = scalars(field::TAGS, S::Tag);
const MENTIONS: Field = edges(field::MENTIONS);
const SECTIONS: Field = records(field::SECTIONS, K::Section);
const FATE: Field = opt(field::VERIFICATION_FATE, S::Enum(VerificationFate::NAMES));

/// One counter per label prefix, in the order `counters` lists them (AC-786).
const COUNTERS: [Field; LabelPrefix::ALL.len()] = {
    let mut fields = [req("P", S::Integer); LabelPrefix::ALL.len()];
    let mut index = 0;
    while index < fields.len() {
        fields[index] = req(LabelPrefix::ALL[index].as_str(), S::Integer);
        index += 1;
    }
    fields
};

const MANIFEST_FIELDS: &[Field] = &[
    req(field::FORMAT, S::Identifier),
    req(field::ALGORITHM, S::Identifier),
    req(field::DOCUMENT_KEY, S::Key),
    opt(field::PUBLISH_BRANCH, S::Identifier),
    scalars(field::PRODUCTS, S::Key),
    record(field::DERIVED_FROM, false, K::DerivedFrom),
];

const DERIVED_FROM_FIELDS: &[Field] = &[
    req(field::AUTHORED_STATE_ID, S::Key),
    req(field::CONTENT_ADDRESS, S::Digest),
    req(field::PUBLISH_SEQUENCE, S::Integer),
];

const LABELS_FIELDS: &[Field] = &[
    record(field::COUNTERS, true, K::Counters),
    ordered(records(field::BINDINGS, K::Binding), Order::ByKey),
];

const BINDING_FIELDS: &[Field] = &[
    req(field::KEY, S::Key),
    req(field::LABEL, S::Label),
    scalars(field::ALIASES, S::Label),
];

const TAG_ENTRY_FIELDS: &[Field] = &[
    KEY,
    req(field::NAME, S::Tag),
    req(field::DEFINITION, S::Text),
];

const CATEGORY_ENTRY_FIELDS: &[Field] = &[
    KEY,
    req(field::NAME, S::Category),
    req(field::DEFINITION, S::Text),
];

const PRODUCT_FIELDS: &[Field] = &[
    KEY,
    SLUG,
    NAME,
    BODY,
    TAGS,
    MENTIONS,
    records(field::FEATURE_GROUPS, K::FeatureGroup),
    SECTIONS,
];

const FEATURE_GROUP_FIELDS: &[Field] = &[
    KEY,
    GROUP_SLUG,
    NAME,
    BODY,
    TAGS,
    MENTIONS,
    scalars(field::FEATURES, S::Key),
    records(field::CONSTRAINTS, K::Constraint),
    SECTIONS,
];

const FEATURE_FIELDS: &[Field] = &[
    KEY,
    SLUG,
    NAME,
    BODY,
    TAGS,
    edges(field::REALIZES),
    MENTIONS,
    records(field::REQUIREMENTS, K::Requirement),
    records(field::QUALITY_ATTRIBUTES, K::QualityAttribute),
    SECTIONS,
];

const REQUIREMENT_FIELDS: &[Field] = &[
    KEY,
    record(field::STATEMENT, true, K::Statement),
    BODY,
    TAGS,
    MENTIONS,
    records(field::CRITERIA, K::Criterion),
    SECTIONS,
];

const STATEMENT_FIELDS: &[Field] = &[
    opt(field::TEXT, S::Text),
    opt(field::FORM, S::Enum(Form::NAMES)),
    opt(field::WHEN, S::Text),
    opt(field::WHILE, S::Text),
    opt(field::IF, S::Text),
    opt(field::WHERE, S::Text),
    opt(field::SYSTEM, S::Text),
    opt(field::SHALL, S::Text),
];

const CRITERION_FIELDS: &[Field] = &[KEY, NAME, BODY, TAGS, MENTIONS, SECTIONS];

const CONSTRAINT_FIELDS: &[Field] = &[
    KEY,
    NAME,
    BODY,
    TAGS,
    record(field::CONSTRAINT_BINDING, false, K::ConstraintBinding),
    opt(field::CONSERVATION_KIND, S::Enum(ConservationKind::NAMES)),
    FATE,
    MENTIONS,
    SECTIONS,
];

const CONSTRAINT_BINDING_FIELDS: &[Field] = &[
    req(field::CATEGORY, S::Category),
    req(field::REQUIRED_VALUE, S::Text),
];

const QUALITY_ATTRIBUTE_FIELDS: &[Field] = &[
    KEY,
    NAME,
    BODY,
    TAGS,
    req(field::QA_KIND, S::Enum(QaKind::NAMES)),
    record(field::QUALITY_TARGET, true, K::QualityTarget),
    FATE,
    MENTIONS,
    SECTIONS,
];

const QUALITY_TARGET_FIELDS: &[Field] = &[
    req(field::METRIC, S::Text),
    record(field::BAND, true, K::Band),
    req(field::UNIT, S::Text),
    req(field::AGGREGATION, S::Enum(QualityAggregation::NAMES)),
    req(field::SCENARIO, S::Text),
    req(field::ENVIRONMENT, S::Text),
];

const BAND_FIELDS: &[Field] = &[
    req(field::KIND, S::Enum(BandKind::NAMES)),
    opt(field::LOWER, S::Text),
    opt(field::UPPER, S::Text),
    opt(field::VALUE, S::Text),
];

const SECTION_FIELDS: &[Field] = &[KEY, NAME, req(field::BODY, S::Text), MENTIONS, SECTIONS];

const DECISION_FIELDS: &[Field] = &[
    KEY,
    SLUG,
    req(field::TITLE, S::Text),
    req(field::CONTEXT, S::Text),
    record(field::STATE, true, K::State),
    records(field::DRIVERS, K::Driver),
    records(field::OPTIONS, K::Option),
    records(field::ASSESSMENTS, K::Assessment),
    records(field::CONSEQUENCES, K::Consequence),
    ordered(
        records(field::SUBJECTS, K::Subject),
        Order::ByRoleThenTarget,
    ),
    MENTIONS,
];

const STATE_FIELDS: &[Field] = &[
    req(field::KIND, S::Enum(StateKind::NAMES)),
    opt(field::SELECTED_OPTION, S::Key),
    opt(field::OUTCOME, S::Text),
    opt(field::RATIONALE, S::Text),
    opt(field::SUCCESSOR, S::Key),
];

const DRIVER_FIELDS: &[Field] = &[
    KEY,
    req(field::KIND, S::Enum(DriverKind::NAMES)),
    opt(field::LABEL, S::Text),
    opt(field::STATEMENT, S::Text),
    node_ref(field::SUBJECT, false),
];

const OPTION_FIELDS: &[Field] = &[
    KEY,
    req(field::TITLE, S::Text),
    req(field::DESCRIPTION, S::Text),
];

const ASSESSMENT_FIELDS: &[Field] = &[
    req(field::OPTION, S::Key),
    req(field::DRIVER, S::Key),
    req(field::RATING, S::Enum(Rating::NAMES)),
    req(field::RATIONALE, S::Text),
];

const CONSEQUENCE_FIELDS: &[Field] = &[
    KEY,
    req(field::KIND, S::Enum(ConsequenceKind::NAMES)),
    req(field::STATEMENT, S::Text),
    node_ref(field::SUBJECT, false),
];

const SUBJECT_FIELDS: &[Field] = &[
    req(field::ROLE, S::Enum(SubjectRole::NAMES)),
    node_ref(field::NODE, true),
];

const FORMER_LABEL_FIELDS: &[Field] = &[req(field::FORMER_LABEL, S::Label)];

impl Kind {
    /// The kind's fields in the Format's order.
    #[must_use]
    pub const fn fields(self) -> &'static [Field] {
        match self {
            Self::Manifest => MANIFEST_FIELDS,
            Self::DerivedFrom => DERIVED_FROM_FIELDS,
            Self::Labels => LABELS_FIELDS,
            Self::Counters => &COUNTERS,
            Self::Binding => BINDING_FIELDS,
            Self::TagEntry => TAG_ENTRY_FIELDS,
            Self::CategoryEntry => CATEGORY_ENTRY_FIELDS,
            Self::Product => PRODUCT_FIELDS,
            Self::FeatureGroup => FEATURE_GROUP_FIELDS,
            Self::Feature => FEATURE_FIELDS,
            Self::Requirement => REQUIREMENT_FIELDS,
            Self::Statement => STATEMENT_FIELDS,
            Self::Criterion => CRITERION_FIELDS,
            Self::RootComponent => &ROOT_COMPONENT,
            Self::Component => &COMPONENT,
            Self::Constraint => CONSTRAINT_FIELDS,
            Self::ConstraintBinding => CONSTRAINT_BINDING_FIELDS,
            Self::QualityAttribute => QUALITY_ATTRIBUTE_FIELDS,
            Self::QualityTarget => QUALITY_TARGET_FIELDS,
            Self::Band => BAND_FIELDS,
            Self::Section => SECTION_FIELDS,
            Self::Decision => DECISION_FIELDS,
            Self::State => STATE_FIELDS,
            Self::Driver => DRIVER_FIELDS,
            Self::Option => OPTION_FIELDS,
            Self::Assessment => ASSESSMENT_FIELDS,
            Self::Consequence => CONSEQUENCE_FIELDS,
            Self::Subject => SUBJECT_FIELDS,
            Self::FormerLabel => FORMER_LABEL_FIELDS,
        }
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

const COMPONENT: [Field; 10] = [
    KEY,
    NAME,
    BODY,
    TAGS,
    edges(field::SCOPED_TO),
    MENTIONS,
    records(field::COMPONENTS, K::Component),
    records(field::CONSTRAINTS, K::Constraint),
    records(field::QUALITY_ATTRIBUTES, K::QualityAttribute),
    SECTIONS,
];
/// A root Component: `slug` after `key`, then a Component's fields (AC-757).
const ROOT_COMPONENT: [Field; 11] = {
    let mut fields = [KEY; 11];
    fields[1] = SLUG;
    let mut index = 1;
    while index < COMPONENT.len() {
        fields[index + 1] = COMPONENT[index];
        index += 1;
    }
    fields
};

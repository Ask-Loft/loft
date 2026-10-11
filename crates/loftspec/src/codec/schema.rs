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

/// What a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Scalar(Scalar),
    Record(Kind),
    /// A node's key, or a former label as `{formerLabel: <label>}` (AC-809).
    NodeRef,
    List(Item),
}

/// What each entry of a list holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Scalar(Scalar),
    Record(Kind),
    NodeRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub required: bool,
    pub shape: Shape,
}

const fn field(name: &'static str, required: bool, shape: Shape) -> Field {
    Field {
        name,
        required,
        shape,
    }
}

const fn req(name: &'static str, scalar: Scalar) -> Field {
    field(name, true, Shape::Scalar(scalar))
}

const fn opt(name: &'static str, scalar: Scalar) -> Field {
    field(name, false, Shape::Scalar(scalar))
}

const fn record(name: &'static str, required: bool, kind: Kind) -> Field {
    field(name, required, Shape::Record(kind))
}

const fn records(name: &'static str, kind: Kind) -> Field {
    field(name, false, Shape::List(Item::Record(kind)))
}

const fn scalars(name: &'static str, scalar: Scalar) -> Field {
    field(name, false, Shape::List(Item::Scalar(scalar)))
}

pub const FORMS: &[&str] = &[
    "ubiquitous",
    "event_driven",
    "state_driven",
    "unwanted",
    "optional",
];
pub const STATES: &[&str] = &["proposed", "accepted", "rejected", "superseded", "retired"];
pub const DRIVER_KINDS: &[&str] = &["local", "subject"];
pub const RATINGS: &[&str] = &["supports", "neutral", "conflicts", "unknown"];
pub const CONSEQUENCE_KINDS: &[&str] = &["benefit", "cost", "risk", "obligation"];
pub const ROLES: &[&str] = &["outcome", "affected"];
pub const QA_KINDS: &[&str] = &[
    "latency",
    "throughput",
    "availability",
    "cost",
    "testability",
    "usability",
    "capacity",
];
pub const AGGREGATIONS: &[&str] = &[
    "each", "single", "minimum", "maximum", "mean", "median", "p95", "p99", "total",
];
pub const BAND_KINDS: &[&str] = &["at_most", "at_least", "exactly", "between"];
pub const CONSERVATION_KINDS: &[&str] = &["sum_preservation", "cap", "operation_identity"];
pub const FATES: &[&str] = &["proved", "monitored", "sampled"];

/// The label prefixes, in the order `counters` lists them (AC-786).
pub const PREFIXES: &[&str] = &["P", "FG", "F", "RQ", "AC", "CMP", "CON", "QA", "ADR"];

use Kind as K;
use Scalar as S;

const KEY: Field = req("key", S::Key);
const SLUG: Field = req("slug", S::Slug);
const NAME: Field = req("name", S::Text);
const BODY: Field = opt("body", S::Text);
const TAGS: Field = scalars("tags", S::Tag);
const MENTIONS: Field = scalars("mentions", S::Key);
const SECTIONS: Field = records("sections", K::Section);
const FATE: Field = opt("verificationFate", S::Enum(FATES));

const COUNTERS: [Field; 9] = {
    let mut fields = [req("P", S::Integer); 9];
    let mut index = 0;
    while index < PREFIXES.len() {
        fields[index] = req(PREFIXES[index], S::Integer);
        index += 1;
    }
    fields
};

const MANIFEST_FIELDS: &[Field] = &[
    req("format", S::Identifier),
    req("algorithm", S::Identifier),
    req("documentKey", S::Key),
    opt("publishBranch", S::Identifier),
    scalars("products", S::Key),
    record("derivedFrom", false, K::DerivedFrom),
];

const DERIVED_FROM_FIELDS: &[Field] = &[
    req("authoredStateId", S::Key),
    req("contentAddress", S::Digest),
    req("publishSequence", S::Integer),
];

const LABELS_FIELDS: &[Field] = &[
    record("counters", true, K::Counters),
    records("bindings", K::Binding),
];

const BINDING_FIELDS: &[Field] = &[
    req("key", S::Key),
    req("label", S::Label),
    scalars("aliases", S::Label),
];

const TAG_ENTRY_FIELDS: &[Field] = &[KEY, req("name", S::Tag), req("definition", S::Text)];

const CATEGORY_ENTRY_FIELDS: &[Field] =
    &[KEY, req("name", S::Category), req("definition", S::Text)];

const PRODUCT_FIELDS: &[Field] = &[
    KEY,
    SLUG,
    NAME,
    BODY,
    TAGS,
    MENTIONS,
    records("featureGroups", K::FeatureGroup),
    SECTIONS,
];

const FEATURE_GROUP_FIELDS: &[Field] = &[
    KEY,
    SLUG,
    NAME,
    BODY,
    TAGS,
    MENTIONS,
    scalars("features", S::Key),
    records("constraints", K::Constraint),
    SECTIONS,
];

const FEATURE_FIELDS: &[Field] = &[
    KEY,
    SLUG,
    NAME,
    BODY,
    TAGS,
    scalars("realizes", S::Key),
    MENTIONS,
    records("requirements", K::Requirement),
    records("qualityAttributes", K::QualityAttribute),
    SECTIONS,
];

const REQUIREMENT_FIELDS: &[Field] = &[
    KEY,
    record("statement", true, K::Statement),
    BODY,
    TAGS,
    MENTIONS,
    records("criteria", K::Criterion),
    SECTIONS,
];

const STATEMENT_FIELDS: &[Field] = &[
    opt("text", S::Text),
    opt("form", S::Enum(FORMS)),
    opt("when", S::Text),
    opt("while", S::Text),
    opt("if", S::Text),
    opt("where", S::Text),
    opt("system", S::Text),
    opt("shall", S::Text),
];

const CRITERION_FIELDS: &[Field] = &[KEY, NAME, BODY, TAGS, MENTIONS, SECTIONS];

const CONSTRAINT_FIELDS: &[Field] = &[
    KEY,
    NAME,
    BODY,
    TAGS,
    record("constraintBinding", false, K::ConstraintBinding),
    opt("conservationKind", S::Enum(CONSERVATION_KINDS)),
    FATE,
    MENTIONS,
    SECTIONS,
];

const CONSTRAINT_BINDING_FIELDS: &[Field] =
    &[req("category", S::Category), req("requiredValue", S::Text)];

const QUALITY_ATTRIBUTE_FIELDS: &[Field] = &[
    KEY,
    NAME,
    BODY,
    TAGS,
    req("qaKind", S::Enum(QA_KINDS)),
    record("qualityTarget", true, K::QualityTarget),
    FATE,
    MENTIONS,
    SECTIONS,
];

const QUALITY_TARGET_FIELDS: &[Field] = &[
    req("metric", S::Text),
    record("band", true, K::Band),
    req("unit", S::Text),
    req("aggregation", S::Enum(AGGREGATIONS)),
    req("scenario", S::Text),
    req("environment", S::Text),
];

const BAND_FIELDS: &[Field] = &[
    req("kind", S::Enum(BAND_KINDS)),
    opt("lower", S::Text),
    opt("upper", S::Text),
    opt("value", S::Text),
];

const SECTION_FIELDS: &[Field] = &[KEY, NAME, req("body", S::Text), MENTIONS, SECTIONS];

const DECISION_FIELDS: &[Field] = &[
    KEY,
    SLUG,
    req("title", S::Text),
    req("context", S::Text),
    record("state", true, K::State),
    records("drivers", K::Driver),
    records("options", K::Option),
    records("assessments", K::Assessment),
    records("consequences", K::Consequence),
    records("subjects", K::Subject),
    MENTIONS,
];

const STATE_FIELDS: &[Field] = &[
    req("kind", S::Enum(STATES)),
    opt("selectedOption", S::Key),
    opt("outcome", S::Text),
    opt("rationale", S::Text),
    opt("successor", S::Key),
];

const DRIVER_FIELDS: &[Field] = &[
    KEY,
    req("kind", S::Enum(DRIVER_KINDS)),
    opt("label", S::Text),
    opt("statement", S::Text),
    field("subject", false, Shape::NodeRef),
];

const OPTION_FIELDS: &[Field] = &[KEY, req("title", S::Text), req("description", S::Text)];

const ASSESSMENT_FIELDS: &[Field] = &[
    req("option", S::Key),
    req("driver", S::Key),
    req("rating", S::Enum(RATINGS)),
    req("rationale", S::Text),
];

const CONSEQUENCE_FIELDS: &[Field] = &[
    KEY,
    req("kind", S::Enum(CONSEQUENCE_KINDS)),
    req("statement", S::Text),
    field("subject", false, Shape::NodeRef),
];

const SUBJECT_FIELDS: &[Field] = &[
    req("role", S::Enum(ROLES)),
    field("node", true, Shape::NodeRef),
];

const FORMER_LABEL_FIELDS: &[Field] = &[req("formerLabel", S::Label)];

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

    /// Whether the record carries a key the formatter mints when it is missing (RQ-3, RQ-218).
    #[must_use]
    pub const fn is_keyed(self) -> bool {
        matches!(
            self,
            Self::TagEntry
                | Self::CategoryEntry
                | Self::Product
                | Self::FeatureGroup
                | Self::Feature
                | Self::Requirement
                | Self::Criterion
                | Self::RootComponent
                | Self::Component
                | Self::Constraint
                | Self::QualityAttribute
                | Self::Section
                | Self::Decision
                | Self::Driver
                | Self::Option
                | Self::Consequence
        )
    }
}

const COMPONENT: [Field; 10] = [
    KEY,
    NAME,
    BODY,
    TAGS,
    scalars("scopedTo", S::Key),
    MENTIONS,
    records("components", K::Component),
    records("constraints", K::Constraint),
    records("qualityAttributes", K::QualityAttribute),
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

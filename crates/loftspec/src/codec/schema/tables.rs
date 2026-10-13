//! The field tables of the record schema: each kind's fields in the Format's order
//! (F-49), built with the constructors below.

use super::{Field, Item, Kind, Order, Scalar, Shape, Supply, field};

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

use crate::attributes::{
    BandKind, Boundary, ConservationKind, QaKind, QualityAggregation, VerificationFate,
};
use crate::decision::{ConsequenceKind, DriverKind, Rating, StateKind};
use crate::document::SubjectRole;
use crate::identity::LabelPrefix;
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
    scalars(field::WITHDRAWN, S::Label),
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
    records(field::CONSTRAINTS, K::Constraint),
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

const COMPONENT: [Field; 11] = [
    KEY,
    NAME,
    BODY,
    TAGS,
    opt(field::BOUNDARY, S::Enum(Boundary::NAMES)),
    edges(field::SCOPED_TO),
    MENTIONS,
    records(field::COMPONENTS, K::Component),
    records(field::CONSTRAINTS, K::Constraint),
    records(field::QUALITY_ATTRIBUTES, K::QualityAttribute),
    SECTIONS,
];
/// A root Component: `slug` after `key`, then a Component's fields (AC-757).
const ROOT_COMPONENT: [Field; 12] = {
    let mut fields = [KEY; 12];
    fields[1] = SLUG;
    let mut index = 1;
    while index < COMPONENT.len() {
        fields[index + 1] = COMPONENT[index];
        index += 1;
    }
    fields
};

/// The fields of `kind` in the Format's order.
pub(super) const fn fields(kind: Kind) -> &'static [Field] {
    match kind {
        Kind::Manifest => MANIFEST_FIELDS,
        Kind::DerivedFrom => DERIVED_FROM_FIELDS,
        Kind::Labels => LABELS_FIELDS,
        Kind::Counters => &COUNTERS,
        Kind::Binding => BINDING_FIELDS,
        Kind::TagEntry => TAG_ENTRY_FIELDS,
        Kind::CategoryEntry => CATEGORY_ENTRY_FIELDS,
        Kind::Product => PRODUCT_FIELDS,
        Kind::FeatureGroup => FEATURE_GROUP_FIELDS,
        Kind::Feature => FEATURE_FIELDS,
        Kind::Requirement => REQUIREMENT_FIELDS,
        Kind::Statement => STATEMENT_FIELDS,
        Kind::Criterion => CRITERION_FIELDS,
        Kind::RootComponent => &ROOT_COMPONENT,
        Kind::Component => &COMPONENT,
        Kind::Constraint => CONSTRAINT_FIELDS,
        Kind::ConstraintBinding => CONSTRAINT_BINDING_FIELDS,
        Kind::QualityAttribute => QUALITY_ATTRIBUTE_FIELDS,
        Kind::QualityTarget => QUALITY_TARGET_FIELDS,
        Kind::Band => BAND_FIELDS,
        Kind::Section => SECTION_FIELDS,
        Kind::Decision => DECISION_FIELDS,
        Kind::State => STATE_FIELDS,
        Kind::Driver => DRIVER_FIELDS,
        Kind::Option => OPTION_FIELDS,
        Kind::Assessment => ASSESSMENT_FIELDS,
        Kind::Consequence => CONSEQUENCE_FIELDS,
        Kind::Subject => SUBJECT_FIELDS,
        Kind::FormerLabel => FORMER_LABEL_FIELDS,
    }
}

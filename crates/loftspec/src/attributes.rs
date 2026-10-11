//! The typed attributes of Constraints and Quality Attributes (AC-34): the Quality Attribute kind
//! and target, the technical binding, the conservation kind and the verification fate. Their
//! fields are those of a Loft v1 export, kept field for field (AC-165, AC-166).

use serde::Serialize;

use crate::names::CategoryName;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum QaKind {
    Latency,
    Throughput,
    Availability,
    Cost,
    Testability,
    Usability,
    Capacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityAggregation {
    Each,
    Single,
    Minimum,
    Maximum,
    Mean,
    Median,
    P95,
    P99,
    Total,
}

/// The accepted range of a measured value; bounds are decimal strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QualityBand {
    AtMost { upper: String },
    AtLeast { lower: String },
    Exactly { value: String },
    Between { lower: String, upper: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityTarget {
    pub metric: String,
    pub band: QualityBand,
    pub unit: String,
    pub aggregation: QualityAggregation,
    pub scenario: String,
    pub environment: String,
}

/// The value a Constraint requires within a constraint category.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalBinding {
    pub category: CategoryName,
    pub required_value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConservationKind {
    SumPreservation,
    Cap,
    OperationIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationFate {
    Proved,
    Monitored,
    Sampled,
}

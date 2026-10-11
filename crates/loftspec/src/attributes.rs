//! The typed attributes of Constraints and Quality Attributes (AC-596): the Quality Attribute kind
//! and target, the technical binding, the conservation kind and the verification fate. Their
//! fields are those of a Loft v1 export, kept field for field (AC-165, AC-166).

use serde::{Deserialize, Serialize};

use crate::names::CategoryName;
use crate::wire::wire_names;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

wire_names!(QaKind {
    Latency => "latency",
    Throughput => "throughput",
    Availability => "availability",
    Cost => "cost",
    Testability => "testability",
    Usability => "usability",
    Capacity => "capacity",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

wire_names!(QualityAggregation {
    Each => "each",
    Single => "single",
    Minimum => "minimum",
    Maximum => "maximum",
    Mean => "mean",
    Median => "median",
    P95 => "p95",
    P99 => "p99",
    Total => "total",
});

/// The accepted range of a measured value; bounds are decimal strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QualityBand {
    AtMost { upper: String },
    AtLeast { lower: String },
    Exactly { value: String },
    Between { lower: String, upper: String },
}

/// The kind of a quality band, without its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BandKind {
    AtMost,
    AtLeast,
    Exactly,
    Between,
}

wire_names!(BandKind {
    AtMost => "at_most",
    AtLeast => "at_least",
    Exactly => "exactly",
    Between => "between",
});

impl QualityBand {
    #[must_use]
    pub const fn kind(&self) -> BandKind {
        match self {
            Self::AtMost { .. } => BandKind::AtMost,
            Self::AtLeast { .. } => BandKind::AtLeast,
            Self::Exactly { .. } => BandKind::Exactly,
            Self::Between { .. } => BandKind::Between,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityTarget {
    pub metric: String,
    pub band: QualityBand,
    pub unit: String,
    pub aggregation: QualityAggregation,
    pub scenario: String,
    pub environment: String,
}

/// The value a Constraint requires within a constraint category.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TechnicalBinding {
    pub category: CategoryName,
    pub required_value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConservationKind {
    SumPreservation,
    Cap,
    OperationIdentity,
}

wire_names!(ConservationKind {
    SumPreservation => "sum_preservation",
    Cap => "cap",
    OperationIdentity => "operation_identity",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationFate {
    Proved,
    Monitored,
    Sampled,
}

wire_names!(VerificationFate {
    Proved => "proved",
    Monitored => "monitored",
    Sampled => "sampled",
});

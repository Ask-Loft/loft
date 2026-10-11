//! Architecture Decisions: drivers, options, the assessment matrix and consequences, each element
//! keyed (AC-181) and kept field for field from a v1 export (AC-257).

use serde::Deserialize;

use crate::identity::Label;
use crate::ids::{ConsequenceId, DecisionId, DriverId, NodeId, OptionId};
use crate::slug::Slug;

/// A node a decision names: one the document holds, or, on a retired or superseded decision, one
/// it no longer holds, named by its former label (ADR-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeRef {
    Held(NodeId),
    Former(Label),
}

impl<T: Into<NodeId>> From<T> for NodeRef {
    fn from(node: T) -> Self {
        Self::Held(node.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub id: DecisionId,
    pub slug: Slug,
    pub title: String,
    pub context: String,
    pub state: DecisionState,
    pub drivers: Vec<Driver>,
    pub options: Vec<DecisionOption>,
    pub assessments: Vec<Assessment>,
    pub consequences: Vec<Consequence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionState {
    Proposed,
    Accepted {
        selected_option: OptionId,
        outcome: String,
    },
    Rejected {
        rationale: String,
    },
    Superseded {
        selected_option: OptionId,
        outcome: String,
        successor: DecisionId,
    },
    Retired {
        selected_option: OptionId,
        outcome: String,
        rationale: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Driver {
    pub id: DriverId,
    pub payload: DriverPayload,
}

/// A driver stated in the decision, or one that names a node of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverPayload {
    Local { label: String, statement: String },
    Subject { subject: NodeRef },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionOption {
    pub id: OptionId,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Supports,
    Neutral,
    Conflicts,
    Unknown,
}

impl Rating {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supports => "supports",
            Self::Neutral => "neutral",
            Self::Conflicts => "conflicts",
            Self::Unknown => "unknown",
        }
    }
}

/// One cell of the option × driver matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    pub option: OptionId,
    pub driver: DriverId,
    pub rating: Rating,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsequenceKind {
    Benefit,
    Cost,
    Risk,
    Obligation,
}

impl ConsequenceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Benefit => "benefit",
            Self::Cost => "cost",
            Self::Risk => "risk",
            Self::Obligation => "obligation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consequence {
    pub id: ConsequenceId,
    pub kind: ConsequenceKind,
    pub statement: String,
    pub subject: Option<NodeRef>,
}

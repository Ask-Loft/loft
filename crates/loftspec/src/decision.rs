//! Architecture Decisions: drivers, options, the assessment matrix and consequences, each element
//! keyed (AC-181) and kept field for field from a v1 export (AC-257).

use crate::wire::wire_names;
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

/// The kind of a decision state, without its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateKind {
    Proposed,
    Accepted,
    Rejected,
    Superseded,
    Retired,
}

wire_names!(StateKind {
    Proposed => "proposed",
    Accepted => "accepted",
    Rejected => "rejected",
    Superseded => "superseded",
    Retired => "retired",
});

impl DecisionState {
    #[must_use]
    pub const fn kind(&self) -> StateKind {
        match self {
            Self::Proposed => StateKind::Proposed,
            Self::Accepted { .. } => StateKind::Accepted,
            Self::Rejected { .. } => StateKind::Rejected,
            Self::Superseded { .. } => StateKind::Superseded,
            Self::Retired { .. } => StateKind::Retired,
        }
    }
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

/// The kind of a driver, without its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DriverKind {
    Local,
    Subject,
}

wire_names!(DriverKind {
    Local => "local",
    Subject => "subject",
});

impl DriverPayload {
    #[must_use]
    pub const fn kind(&self) -> DriverKind {
        match self {
            Self::Local { .. } => DriverKind::Local,
            Self::Subject { .. } => DriverKind::Subject,
        }
    }
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

wire_names!(Rating {
    Supports => "supports",
    Neutral => "neutral",
    Conflicts => "conflicts",
    Unknown => "unknown",
});

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

wire_names!(ConsequenceKind {
    Benefit => "benefit",
    Cost => "cost",
    Risk => "risk",
    Obligation => "obligation",
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consequence {
    pub id: ConsequenceId,
    pub kind: ConsequenceKind,
    pub statement: String,
    pub subject: Option<NodeRef>,
}

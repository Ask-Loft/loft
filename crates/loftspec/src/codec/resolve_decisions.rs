//! Decision records as typed decisions, with their `subject` edges (AC-806).

use super::error::ReadError;
use super::record::Rec;
use super::resolve::{Resolver, key, slug, text, value};
use crate::decision::{
    Assessment, Consequence, Decision, DecisionOption, DecisionState, Driver, DriverPayload,
};
use crate::document::Edge;
use crate::ids::{ConsequenceId, DecisionId, DriverId, ElementId, OptionId};

impl Resolver<'_> {
    pub(super) fn decision(&mut self, record: &Rec) -> Result<Decision, ReadError> {
        let id = DecisionId::new(key(record, "key"));
        self.mentions(record, ElementId::Decision(id))?;
        for subject in record.records("subjects") {
            let node = self.node_ref(subject.get("node").expect("a checked node"))?;
            self.document.edges.push(Edge::Subject {
                decision: id,
                subject: node,
                role: value(&text(subject, "role")),
            });
        }
        let drivers = record
            .records("drivers")
            .map(|driver| {
                let payload = match driver.get("subject") {
                    Some(subject) => DriverPayload::Subject {
                        subject: self.node_ref(subject)?,
                    },
                    None => DriverPayload::Local {
                        label: text(driver, "label"),
                        statement: text(driver, "statement"),
                    },
                };
                Ok(Driver {
                    id: DriverId::new(key(driver, "key")),
                    payload,
                })
            })
            .collect::<Result<_, ReadError>>()?;
        let consequences = record
            .records("consequences")
            .map(|consequence| {
                Ok(Consequence {
                    id: ConsequenceId::new(key(consequence, "key")),
                    kind: value(&text(consequence, "kind")),
                    statement: text(consequence, "statement"),
                    subject: consequence
                        .get("subject")
                        .map(|subject| self.node_ref(subject))
                        .transpose()?,
                })
            })
            .collect::<Result<_, ReadError>>()?;
        Ok(Decision {
            id,
            slug: slug(record),
            title: text(record, "title"),
            context: text(record, "context"),
            state: state_of(record.record("state").expect("a checked state")),
            drivers,
            options: record
                .records("options")
                .map(|option| DecisionOption {
                    id: OptionId::new(key(option, "key")),
                    title: text(option, "title"),
                    description: text(option, "description"),
                })
                .collect(),
            assessments: record
                .records("assessments")
                .map(|assessment| Assessment {
                    option: OptionId::new(key(assessment, "option")),
                    driver: DriverId::new(key(assessment, "driver")),
                    rating: value(&text(assessment, "rating")),
                    rationale: text(assessment, "rationale"),
                })
                .collect(),
            consequences,
        })
    }
}

/// A state record as the decision's state (AC-770..AC-775).
fn state_of(state: &Rec) -> DecisionState {
    let selected = || OptionId::new(key(state, "selectedOption"));
    let field = |name: &str| text(state, name);
    match state.text("kind").unwrap_or_default() {
        "accepted" => DecisionState::Accepted {
            selected_option: selected(),
            outcome: field("outcome"),
        },
        "rejected" => DecisionState::Rejected {
            rationale: field("rationale"),
        },
        "superseded" => DecisionState::Superseded {
            selected_option: selected(),
            outcome: field("outcome"),
            successor: DecisionId::new(key(state, "successor")),
        },
        "retired" => DecisionState::Retired {
            selected_option: selected(),
            outcome: field("outcome"),
            rationale: field("rationale"),
        },
        _ => DecisionState::Proposed,
    }
}

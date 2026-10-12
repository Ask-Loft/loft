//! Decision records as typed decisions, with their `subject` edges (AC-806).

use super::error::ReadError;
use super::record::Rec;
use super::resolve::{Resolver, checked, key, slug, text};
use super::schema::field;
use crate::decision::{
    Assessment, Consequence, ConsequenceKind, Decision, DecisionOption, DecisionState, Driver,
    DriverPayload, Rating, StateKind,
};
use crate::document::Edge;
use crate::document::SubjectRole;
use crate::ids::{ConsequenceId, DecisionId, DriverId, ElementId, OptionId};

impl Resolver<'_> {
    pub(super) fn decision(&mut self, record: &Rec) -> Result<Decision, ReadError> {
        let id = DecisionId::new(key(record, field::KEY));
        self.mentions(record, ElementId::Decision(id))?;
        for subject in record.records(field::SUBJECTS) {
            let node = self.node_ref(subject.get(field::NODE).expect("a checked node"))?;
            self.document.edges.push(Edge::Subject {
                decision: id,
                subject: node,
                role: checked(SubjectRole::parse, subject, field::ROLE),
            });
        }
        let drivers = record
            .records(field::DRIVERS)
            .map(|driver| {
                let payload = match driver.get(field::SUBJECT) {
                    Some(subject) => DriverPayload::Subject {
                        subject: self.node_ref(subject)?,
                    },
                    None => DriverPayload::Local {
                        label: text(driver, field::LABEL),
                        statement: text(driver, field::STATEMENT),
                    },
                };
                Ok(Driver {
                    id: DriverId::new(key(driver, field::KEY)),
                    payload,
                })
            })
            .collect::<Result<_, ReadError>>()?;
        let consequences = record
            .records(field::CONSEQUENCES)
            .map(|consequence| {
                Ok(Consequence {
                    id: ConsequenceId::new(key(consequence, field::KEY)),
                    kind: checked(ConsequenceKind::parse, consequence, field::KIND),
                    statement: text(consequence, field::STATEMENT),
                    subject: consequence
                        .get(field::SUBJECT)
                        .map(|subject| self.node_ref(subject))
                        .transpose()?,
                })
            })
            .collect::<Result<_, ReadError>>()?;
        Ok(Decision {
            id,
            slug: slug(record),
            title: text(record, field::TITLE),
            context: text(record, field::CONTEXT),
            state: state_of(record.record(field::STATE).expect("a checked state")),
            drivers,
            options: record
                .records(field::OPTIONS)
                .map(|option| DecisionOption {
                    id: OptionId::new(key(option, field::KEY)),
                    title: text(option, field::TITLE),
                    description: text(option, field::DESCRIPTION),
                })
                .collect(),
            assessments: record
                .records(field::ASSESSMENTS)
                .map(|assessment| Assessment {
                    option: OptionId::new(key(assessment, field::OPTION)),
                    driver: DriverId::new(key(assessment, field::DRIVER)),
                    rating: checked(Rating::parse, assessment, field::RATING),
                    rationale: text(assessment, field::RATIONALE),
                })
                .collect(),
            consequences,
        })
    }
}

/// A state record as the decision's state (AC-770..AC-775).
fn state_of(state: &Rec) -> DecisionState {
    let selected = || OptionId::new(key(state, field::SELECTED_OPTION));
    let field = |name: &str| text(state, name);
    match checked(StateKind::parse, state, field::KIND) {
        StateKind::Proposed => DecisionState::Proposed,
        StateKind::Accepted => DecisionState::Accepted {
            selected_option: selected(),
            outcome: field(field::OUTCOME),
        },
        StateKind::Rejected => DecisionState::Rejected {
            rationale: field(field::RATIONALE),
        },
        StateKind::Superseded => DecisionState::Superseded {
            selected_option: selected(),
            outcome: field(field::OUTCOME),
            successor: DecisionId::new(key(state, field::SUCCESSOR)),
        },
        StateKind::Retired => DecisionState::Retired {
            selected_option: selected(),
            outcome: field(field::OUTCOME),
            rationale: field(field::RATIONALE),
        },
    }
}

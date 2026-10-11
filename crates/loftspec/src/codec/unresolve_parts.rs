//! The records of a statement, a quality target and a decision's own fields.

use super::record::{Rec, Val};
use super::schema::Kind;
use super::schema::field;
use crate::attributes::{QualityBand, QualityTarget};
use crate::decision::{Decision, DecisionState, DriverPayload, NodeRef};
use crate::statement::Statement;

/// A statement as `text`, or as `form` with its slots in template order (AC-751, AC-752).
pub(super) fn statement(statement: &Statement) -> Rec {
    let mut record = Rec::new(Kind::Statement);
    let form = match statement {
        Statement::Text(text) => {
            record.set_text(field::TEXT, text.as_str());
            return record;
        }
        Statement::Form(form) => form,
    };
    let kind = form.kind();
    record.set_text(field::FORM, kind.as_str());
    let (condition, system, shall) = form.parts();
    if let (Some(slot), Some(text)) = (kind.condition(), condition) {
        record.set_text(slot, text);
    }
    record.set_text(field::SYSTEM, system);
    record.set_text(field::SHALL, shall);
    record
}

/// A quality target with its band (AC-764, AC-765).
pub(super) fn target(target: &QualityTarget) -> Rec {
    let mut band = Rec::new(Kind::Band);
    let (lower, upper, value) = match &target.band {
        QualityBand::AtMost { upper } => (None, Some(upper), None),
        QualityBand::AtLeast { lower } => (Some(lower), None, None),
        QualityBand::Exactly { value } => (None, None, Some(value)),
        QualityBand::Between { lower, upper } => (Some(lower), Some(upper), None),
    };
    band.set_text(field::KIND, target.band.kind().as_str());
    for (field, bound) in [("lower", lower), ("upper", upper), ("value", value)] {
        if let Some(bound) = bound {
            band.set_text(field, bound.as_str());
        }
    }
    let mut record = Rec::new(Kind::QualityTarget);
    record.set_text(field::METRIC, target.metric.as_str());
    record.set(field::BAND, Val::Record(band));
    record.set_text(field::UNIT, target.unit.as_str());
    record.set_text(field::AGGREGATION, target.aggregation.as_str());
    record.set_text(field::SCENARIO, target.scenario.as_str());
    record.set_text(field::ENVIRONMENT, target.environment.as_str());
    record
}

/// A decision's own fields: everything but its subjects and mentions, which are edges.
pub(super) fn decision_fields(decision: &Decision) -> Rec {
    let mut record = Rec::new(Kind::Decision);
    record.set_text(field::KEY, decision.id.key().to_string());
    record.set_text(field::SLUG, decision.slug.to_string());
    record.set_text(field::TITLE, decision.title.as_str());
    record.set_text(field::CONTEXT, decision.context.as_str());
    record.set(field::STATE, Val::Record(state(&decision.state)));
    let drivers = decision.drivers.iter().map(|driver| {
        let mut record = Rec::new(Kind::Driver);
        record.set_text(field::KEY, driver.id.key().to_string());
        driver_payload(&mut record, &driver.payload);
        Val::Record(record)
    });
    record.set(field::DRIVERS, Val::List(drivers.collect()));
    let options = decision.options.iter().map(|option| {
        let mut record = Rec::new(Kind::Option);
        record.set_text(field::KEY, option.id.key().to_string());
        record.set_text(field::TITLE, option.title.as_str());
        record.set_text(field::DESCRIPTION, option.description.as_str());
        Val::Record(record)
    });
    record.set(field::OPTIONS, Val::List(options.collect()));
    let assessments = decision.assessments.iter().map(|assessment| {
        let mut record = Rec::new(Kind::Assessment);
        record.set_text(field::OPTION, assessment.option.key().to_string());
        record.set_text(field::DRIVER, assessment.driver.key().to_string());
        record.set_text(field::RATING, assessment.rating.as_str());
        record.set_text(field::RATIONALE, assessment.rationale.as_str());
        Val::Record(record)
    });
    record.set(field::ASSESSMENTS, Val::List(assessments.collect()));
    let consequences = decision.consequences.iter().map(|consequence| {
        let mut record = Rec::new(Kind::Consequence);
        record.set_text(field::KEY, consequence.id.key().to_string());
        record.set_text(field::KIND, consequence.kind.as_str());
        record.set_text(field::STATEMENT, consequence.statement.as_str());
        if let Some(subject) = consequence.subject {
            record.set(field::SUBJECT, node_ref(subject));
        }
        Val::Record(record)
    });
    record.set(field::CONSEQUENCES, Val::List(consequences.collect()));
    record
}

/// A node reference: a held node's key, or `{formerLabel: <label>}` (AC-809).
pub(super) fn node_ref(subject: NodeRef) -> Val {
    match subject {
        NodeRef::Held(node) => Val::scalar(&node.key()),
        NodeRef::Former(label) => {
            let mut record = Rec::new(Kind::FormerLabel);
            record.set_text(field::FORMER_LABEL, label.to_string());
            Val::Record(record)
        }
    }
}

fn state(state: &DecisionState) -> Rec {
    let mut record = Rec::new(Kind::State);
    let (selected, outcome, rationale, successor) = match state {
        DecisionState::Proposed => (None, None, None, None),
        DecisionState::Accepted {
            selected_option,
            outcome,
        } => (Some(selected_option), Some(outcome), None, None),
        DecisionState::Rejected { rationale } => (None, None, Some(rationale), None),
        DecisionState::Superseded {
            selected_option,
            outcome,
            successor,
        } => (Some(selected_option), Some(outcome), None, Some(successor)),
        DecisionState::Retired {
            selected_option,
            outcome,
            rationale,
        } => (Some(selected_option), Some(outcome), Some(rationale), None),
    };
    record.set_text(field::KIND, state.kind().as_str());
    if let Some(option) = selected {
        record.set_text(field::SELECTED_OPTION, option.key().to_string());
    }
    if let Some(outcome) = outcome {
        record.set_text(field::OUTCOME, outcome.as_str());
    }
    if let Some(rationale) = rationale {
        record.set_text(field::RATIONALE, rationale.as_str());
    }
    if let Some(successor) = successor {
        record.set_text(field::SUCCESSOR, successor.key().to_string());
    }
    record
}

fn driver_payload(record: &mut Rec, payload: &DriverPayload) {
    record.set_text(field::KIND, payload.kind().as_str());
    match payload {
        DriverPayload::Local { label, statement } => {
            record.set_text(field::LABEL, label.as_str());
            record.set_text(field::STATEMENT, statement.as_str());
        }
        DriverPayload::Subject { subject } => record.set(field::SUBJECT, node_ref(*subject)),
    }
}

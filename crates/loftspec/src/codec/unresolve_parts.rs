//! The records of a statement, a quality target and a decision's own fields.

use super::record::{Rec, Val};
use super::schema::Kind;
use super::unresolve::name_of;
use crate::attributes::{QualityBand, QualityTarget};
use crate::decision::{Decision, DecisionState, DriverPayload, NodeRef};
use crate::statement::{EarsForm, Statement};

/// A statement as `text`, or as `form` with its slots in template order (AC-751, AC-752).
pub(super) fn statement(statement: &Statement) -> Rec {
    let mut record = Rec::new(Kind::Statement);
    let form = match statement {
        Statement::Text(text) => {
            record.set_text("text", text.as_str());
            return record;
        }
        Statement::Form(form) => form,
    };
    record.set_text("form", form.name());
    let (condition, system, shall) = match form {
        EarsForm::Ubiquitous { system, shall } => (None, system, shall),
        EarsForm::EventDriven {
            when,
            system,
            shall,
        } => (Some(("when", when)), system, shall),
        EarsForm::StateDriven {
            r#while,
            system,
            shall,
        } => (Some(("while", r#while)), system, shall),
        EarsForm::Unwanted {
            r#if,
            system,
            shall,
        } => (Some(("if", r#if)), system, shall),
        EarsForm::Optional {
            r#where,
            system,
            shall,
        } => (Some(("where", r#where)), system, shall),
    };
    if let Some((slot, text)) = condition {
        record.set_text(slot, text.as_str());
    }
    record.set_text("system", system.as_str());
    record.set_text("shall", shall.as_str());
    record
}

/// A quality target with its band (AC-764, AC-765).
pub(super) fn target(target: &QualityTarget) -> Rec {
    let mut band = Rec::new(Kind::Band);
    let (kind, lower, upper, value) = match &target.band {
        QualityBand::AtMost { upper } => ("at_most", None, Some(upper), None),
        QualityBand::AtLeast { lower } => ("at_least", Some(lower), None, None),
        QualityBand::Exactly { value } => ("exactly", None, None, Some(value)),
        QualityBand::Between { lower, upper } => ("between", Some(lower), Some(upper), None),
    };
    band.set_text("kind", kind);
    for (field, bound) in [("lower", lower), ("upper", upper), ("value", value)] {
        if let Some(bound) = bound {
            band.set_text(field, bound.as_str());
        }
    }
    let mut record = Rec::new(Kind::QualityTarget);
    record.set_text("metric", target.metric.as_str());
    record.set("band", Val::Record(band));
    record.set_text("unit", target.unit.as_str());
    record.set_text("aggregation", name_of(&target.aggregation));
    record.set_text("scenario", target.scenario.as_str());
    record.set_text("environment", target.environment.as_str());
    record
}

/// A decision's own fields: everything but its subjects and mentions, which are edges.
pub(super) fn decision_fields(decision: &Decision) -> Rec {
    let mut record = Rec::new(Kind::Decision);
    record.set_text("key", decision.id.key().to_string());
    record.set_text("slug", decision.slug.to_string());
    record.set_text("title", decision.title.as_str());
    record.set_text("context", decision.context.as_str());
    record.set("state", Val::Record(state(&decision.state)));
    let drivers = decision.drivers.iter().map(|driver| {
        let mut record = Rec::new(Kind::Driver);
        record.set_text("key", driver.id.key().to_string());
        driver_payload(&mut record, &driver.payload);
        Val::Record(record)
    });
    record.set("drivers", Val::List(drivers.collect()));
    let options = decision.options.iter().map(|option| {
        let mut record = Rec::new(Kind::Option);
        record.set_text("key", option.id.key().to_string());
        record.set_text("title", option.title.as_str());
        record.set_text("description", option.description.as_str());
        Val::Record(record)
    });
    record.set("options", Val::List(options.collect()));
    let assessments = decision.assessments.iter().map(|assessment| {
        let mut record = Rec::new(Kind::Assessment);
        record.set_text("option", assessment.option.key().to_string());
        record.set_text("driver", assessment.driver.key().to_string());
        record.set_text("rating", assessment.rating.as_str());
        record.set_text("rationale", assessment.rationale.as_str());
        Val::Record(record)
    });
    record.set("assessments", Val::List(assessments.collect()));
    let consequences = decision.consequences.iter().map(|consequence| {
        let mut record = Rec::new(Kind::Consequence);
        record.set_text("key", consequence.id.key().to_string());
        record.set_text("kind", consequence.kind.as_str());
        record.set_text("statement", consequence.statement.as_str());
        if let Some(subject) = consequence.subject {
            record.set("subject", node_ref(subject));
        }
        Val::Record(record)
    });
    record.set("consequences", Val::List(consequences.collect()));
    record
}

/// A node reference: a held node's key, or `{formerLabel: <label>}` (AC-809).
pub(super) fn node_ref(subject: NodeRef) -> Val {
    match subject {
        NodeRef::Held(node) => Val::scalar(&node.key()),
        NodeRef::Former(label) => {
            let mut record = Rec::new(Kind::FormerLabel);
            record.set_text("formerLabel", label.to_string());
            Val::Record(record)
        }
    }
}

fn state(state: &DecisionState) -> Rec {
    let mut record = Rec::new(Kind::State);
    let (kind, selected, outcome, rationale, successor) = match state {
        DecisionState::Proposed => ("proposed", None, None, None, None),
        DecisionState::Accepted {
            selected_option,
            outcome,
        } => ("accepted", Some(selected_option), Some(outcome), None, None),
        DecisionState::Rejected { rationale } => ("rejected", None, None, Some(rationale), None),
        DecisionState::Superseded {
            selected_option,
            outcome,
            successor,
        } => (
            "superseded",
            Some(selected_option),
            Some(outcome),
            None,
            Some(successor),
        ),
        DecisionState::Retired {
            selected_option,
            outcome,
            rationale,
        } => (
            "retired",
            Some(selected_option),
            Some(outcome),
            Some(rationale),
            None,
        ),
    };
    record.set_text("kind", kind);
    if let Some(option) = selected {
        record.set_text("selectedOption", option.key().to_string());
    }
    if let Some(outcome) = outcome {
        record.set_text("outcome", outcome.as_str());
    }
    if let Some(rationale) = rationale {
        record.set_text("rationale", rationale.as_str());
    }
    if let Some(successor) = successor {
        record.set_text("successor", successor.key().to_string());
    }
    record
}

fn driver_payload(record: &mut Rec, payload: &DriverPayload) {
    match payload {
        DriverPayload::Local { label, statement } => {
            record.set_text("kind", "local");
            record.set_text("label", label.as_str());
            record.set_text("statement", statement.as_str());
        }
        DriverPayload::Subject { subject } => {
            record.set_text("kind", "subject");
            record.set("subject", node_ref(*subject));
        }
    }
}

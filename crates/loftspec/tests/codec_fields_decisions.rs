//! The fields of a decision and of the records it holds: written in the Format's order, and
//! refused by the reader when a required one is missing. Each expected order is the published
//! list, written out here rather than taken from the record schema the writer follows.

use crate::common::fields::{assert_required, fields, without};
use crate::common::full::full_files;
use crate::common::text::{DECISION, path, refused_in, text};
use crate::common::{key, proposed_decision, small_document};
use loftspec::codec::{self, Files, Problem};
use loftspec::decision::DecisionState;
use loftspec::document::Document;
use loftspec::ids::{DecisionId, OptionId};

const STATE: &[&str] = &["state"];
const LOCAL: &[&str] = &["drivers", "0"];
const SUBJECT: &[&str] = &["drivers", "1"];
const OPTION: &[&str] = &["options", "0"];
const ASSESSMENT: &[&str] = &["assessments", "0"];
const CONSEQUENCE: &[&str] = &["consequences", "0"];

/// The small document with its decision in `state`, and a second decision a superseded state
/// can name as its successor.
fn in_state(state: DecisionState) -> Document {
    let mut document = small_document();
    document.decisions[0].state = state;
    document
        .decisions
        .push(proposed_decision(key(50), "keys-are-ulids"));
    document
}

/// The state the decision `keys-are-uuids` reads back with.
fn read_state(files: &Files) -> DecisionState {
    let document = codec::read(files).unwrap();
    let decision = document
        .decisions
        .iter()
        .find(|decision| decision.id == DecisionId::new(key(20)));
    decision.unwrap().state.clone()
}

#[test]
fn a_decision_records_fields_are_written_in_the_published_order() {
    let published = [
        "key",
        "slug",
        "title",
        "context",
        "state",
        "drivers",
        "options",
        "assessments",
        "consequences",
        "subjects",
        "mentions",
    ];
    assert_eq!(fields(&full_files(), DECISION, &[]), published);
}

#[test]
fn a_decision_record_without_its_key_slug_title_context_or_state_is_refused() {
    for field in ["key", "slug", "title", "context", "state"] {
        assert_required(&full_files(), DECISION, &[], field);
    }
}

#[test]
fn a_state_begins_with_its_kind_and_each_of_the_five_kinds_reads_back() {
    let option = || OptionId::new(key(23));
    let states = [
        (DecisionState::Proposed, "proposed"),
        (
            DecisionState::Accepted {
                selected_option: option(),
                outcome: "Chosen.".to_owned(),
            },
            "accepted",
        ),
        (
            DecisionState::Rejected {
                rationale: "Not needed.".to_owned(),
            },
            "rejected",
        ),
        (
            DecisionState::Superseded {
                selected_option: option(),
                outcome: "Chosen.".to_owned(),
                successor: DecisionId::new(key(50)),
            },
            "superseded",
        ),
        (
            DecisionState::Retired {
                selected_option: option(),
                outcome: "Chosen.".to_owned(),
                rationale: "No longer needed.".to_owned(),
            },
            "retired",
        ),
    ];
    for (state, kind) in states {
        let files = codec::write(&in_state(state.clone()));
        assert_eq!(fields(&files, DECISION, STATE)[0], "kind");
        assert!(
            text(&files, DECISION).contains(&format!("state:\n  kind: {kind}\n")),
            "{kind}"
        );
        assert_eq!(read_state(&files), state);
    }
}

#[test]
fn a_proposed_state_holds_its_kind_alone() {
    let files = codec::write(&in_state(DecisionState::Proposed));
    assert_eq!(fields(&files, DECISION, STATE), ["kind"]);
    assert_eq!(read_state(&files), DecisionState::Proposed);
    let mut files = files;
    let carrying = text(&files, DECISION).replacen(
        "  kind: proposed\n",
        "  kind: proposed\n  outcome: \"Chosen.\"\n",
        1,
    );
    files.insert(path(DECISION), carrying.into_bytes());
    assert!(matches!(
        refused_in(&files, DECISION).problem,
        Problem::Shape(_)
    ));
}

#[test]
fn a_rejected_state_holds_its_kind_and_its_rationale() {
    let rejected = DecisionState::Rejected {
        rationale: "Not needed.".to_owned(),
    };
    let files = codec::write(&in_state(rejected.clone()));
    assert_eq!(fields(&files, DECISION, STATE), ["kind", "rationale"]);
    assert_eq!(read_state(&files), rejected);
    assert_required(&files, DECISION, STATE, "rationale");
}

#[test]
fn a_local_drivers_fields_are_written_in_the_published_order_and_all_are_required() {
    let published = ["key", "kind", "label", "statement"];
    assert_eq!(fields(&full_files(), DECISION, LOCAL), published);
    for field in published {
        assert_required(&full_files(), DECISION, LOCAL, field);
    }
}

#[test]
fn a_subject_drivers_fields_are_written_in_the_published_order_and_all_are_required() {
    let published = ["key", "kind", "subject"];
    assert_eq!(fields(&full_files(), DECISION, SUBJECT), published);
    for field in published {
        assert_required(&full_files(), DECISION, SUBJECT, field);
    }
}

#[test]
fn an_options_fields_are_written_in_the_published_order_and_all_are_required() {
    let published = ["key", "title", "description"];
    assert_eq!(fields(&full_files(), DECISION, OPTION), published);
    for field in published {
        assert_required(&full_files(), DECISION, OPTION, field);
    }
}

#[test]
fn an_assessments_fields_are_written_in_the_published_order_and_all_are_required() {
    let published = ["option", "driver", "rating", "rationale"];
    assert_eq!(fields(&full_files(), DECISION, ASSESSMENT), published);
    for field in published {
        assert_required(&full_files(), DECISION, ASSESSMENT, field);
    }
}

#[test]
fn a_consequence_records_fields_are_written_in_the_published_order() {
    let published = ["key", "kind", "statement", "subject"];
    assert_eq!(fields(&full_files(), DECISION, CONSEQUENCE), published);
}

#[test]
fn a_consequence_record_without_its_key_kind_or_statement_is_refused() {
    for field in ["key", "kind", "statement"] {
        assert_required(&full_files(), DECISION, CONSEQUENCE, field);
    }
    let (files, _) = without(&full_files(), DECISION, CONSEQUENCE, "subject");
    assert!(codec::read(&files).is_ok(), "a subject is optional");
}

/// Asserts that the decision in `state` writes its state with the fields `published` in that
/// order, reads back in it, and is refused without any field after `kind`.
fn assert_state_fields(state: &DecisionState, published: &[&'static str]) {
    let files = codec::write(&in_state(state.clone()));
    assert_eq!(fields(&files, DECISION, STATE), published);
    assert_eq!(read_state(&files), *state);
    for field in &published[1..] {
        assert_required(&files, DECISION, STATE, field);
    }
}

#[test]
fn an_accepted_state_holds_its_kind_selected_option_and_outcome() {
    let accepted = DecisionState::Accepted {
        selected_option: OptionId::new(key(23)),
        outcome: "Chosen.".to_owned(),
    };
    assert_state_fields(&accepted, &["kind", "selectedOption", "outcome"]);
}

#[test]
fn a_superseded_state_holds_its_kind_selected_option_outcome_and_successor() {
    let superseded = DecisionState::Superseded {
        selected_option: OptionId::new(key(23)),
        outcome: "Chosen.".to_owned(),
        successor: DecisionId::new(key(50)),
    };
    let published = ["kind", "selectedOption", "outcome", "successor"];
    assert_state_fields(&superseded, &published);
}

#[test]
fn a_retired_state_holds_its_kind_selected_option_outcome_and_rationale() {
    let retired = DecisionState::Retired {
        selected_option: OptionId::new(key(23)),
        outcome: "Chosen.".to_owned(),
        rationale: "No longer needed.".to_owned(),
    };
    let published = ["kind", "selectedOption", "outcome", "rationale"];
    assert_state_fields(&retired, &published);
}

#[test]
fn a_subject_records_fields_are_its_role_then_its_node_and_both_are_required() {
    let subject = ["subjects", "0"];
    assert_eq!(fields(&full_files(), DECISION, &subject), ["role", "node"]);
    for field in ["role", "node"] {
        assert_required(&full_files(), DECISION, &subject, field);
    }
}

#[test]
fn a_decisions_supersession_is_recorded_in_its_state_and_not_as_an_edge() {
    let superseded = DecisionState::Superseded {
        selected_option: OptionId::new(key(23)),
        outcome: "Chosen.".to_owned(),
        successor: DecisionId::new(key(50)),
    };
    let document = in_state(superseded.clone());
    let files = codec::write(&document);
    let read = codec::read(&files).unwrap();
    assert_eq!(read_state(&files), superseded);
    assert_eq!(read.edges.len(), document.edges.len(), "no edge was added");
    let successor = key(50).to_string();
    for field in ["supersedes", "supersededBy"] {
        let mut carrying = files.clone();
        let edge = format!("{field}:\n  - {successor}\n");
        carrying.insert(
            path(DECISION),
            (text(&files, DECISION) + &edge).into_bytes(),
        );
        assert_eq!(
            refused_in(&carrying, DECISION).problem,
            Problem::UnknownField(field.to_owned())
        );
    }
}

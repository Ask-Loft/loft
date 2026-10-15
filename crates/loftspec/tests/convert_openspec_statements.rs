//! Statements from `OpenSpec` requirement text: the first paragraph is the statement, the rest
//! and the metadata lines are the body, and a statement is kept as slots when it renders back
//! from them, with its first `SHALL` or `MUST` written as `shall` where that is what it takes.

use crate::common::openspec::{converted, one, requirements};
use loftspec::statement::{Form, Statement};

/// The statement and body of the one requirement whose text is `text`.
fn split(text: &str) -> (Statement, Option<String>) {
    let spec = format!("## Requirements\n\n### Requirement: The one\n{text}\n");
    let conversion = converted(&one(&spec));
    let requirement = &requirements(&conversion)[0];
    (
        requirement.statement.clone(),
        requirement.details.body.clone(),
    )
}

fn form(statement: &Statement) -> Option<Form> {
    match statement {
        Statement::Form(form) => Some(form.kind()),
        Statement::Text(_) => None,
    }
}

#[test]
fn the_first_paragraph_is_the_statement_and_the_rest_the_body() {
    let (statement, body) = split(
        "The system SHALL\n   keep an audit log.\n\nEntries are kept a year.\n\n  - indented",
    );
    assert_eq!(statement.render(), "The system shall keep an audit log.");
    assert_eq!(
        body.as_deref(),
        Some("Entries are kept a year.\n\n  - indented")
    );
}

#[test]
fn a_requirement_without_text_takes_its_name_as_the_statement() {
    let spec = "## Requirements\n\n### Requirement: The system shall audit logins.\n\n#### Scenario: Logged\n- **THEN** logged\n";
    let conversion = converted(&one(spec));
    let requirement = &requirements(&conversion)[0];
    assert_eq!(
        requirement.statement.render(),
        "The system shall audit logins."
    );
    assert_eq!(form(&requirement.statement), Some(Form::Ubiquitous));
    assert_eq!(requirement.details.body, None);
}

#[test]
fn metadata_lines_stay_out_of_the_statement_and_open_the_body() {
    let (statement, body) =
        split("**ID**: REQ-7\nThe system SHALL keep a log.\n**Priority**: high\n\nMore detail.");
    assert_eq!(statement.render(), "The system shall keep a log.");
    assert_eq!(
        body.as_deref(),
        Some("**ID**: REQ-7\n**Priority**: high\n\nMore detail.")
    );
}

#[test]
fn a_statement_that_renders_back_is_kept_as_slots() {
    let (statement, _) = split("The system shall keep a log.");
    assert_eq!(form(&statement), Some(Form::Ubiquitous));
}

#[test]
fn an_upper_case_modal_word_is_written_as_shall_when_that_gives_slots() {
    let (shall, _) = split("The system SHALL keep a log.");
    let Statement::Form(slots) = &shall else {
        panic!("slots");
    };
    assert_eq!(slots.parts(), (None, "The system", "keep a log."));
    let (must, _) = split("When a user signs in, the system MUST record the time.");
    assert_eq!(form(&must), Some(Form::EventDriven));
    assert_eq!(
        must.render(),
        "When a user signs in, the system shall record the time."
    );
}

#[test]
fn only_the_first_modal_word_is_changed() {
    let (statement, _) = split("The system SHALL keep a log and MUST rotate it.");
    assert_eq!(
        statement.render(),
        "The system shall keep a log and MUST rotate it."
    );
}

#[test]
fn an_upper_case_condition_keyword_or_a_lower_case_must_stays_text() {
    let (upper, _) = split("WHEN a user signs in, the system SHALL record the time.");
    assert_eq!(
        upper,
        Statement::Text("WHEN a user signs in, the system SHALL record the time.".to_owned())
    );
    let (lower, _) = split("The system must keep a log.");
    assert_eq!(
        lower,
        Statement::Text("The system must keep a log.".to_owned())
    );
}

#[test]
fn any_other_first_paragraph_is_stored_as_text_verbatim() {
    let (statement, _) = split("Users may export their data at any time.");
    assert_eq!(
        statement,
        Statement::Text("Users may export their data at any time.".to_owned())
    );
}

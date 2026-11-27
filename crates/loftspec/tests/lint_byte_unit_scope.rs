//! `V2-014` (F-21): which units a definition settles, and which texts may give one: a count in
//! parentheses after a multiplied unit gives none, and a decision gives one only where it is
//! accepted, outside the options it did not select.

use crate::common::decisions::{decision, set_state, superseded};
use crate::common::key;
use crate::lint_byte_unit_definitions::{with_text, with_text_stating};
use crate::lint_byte_units::{FEATURE, beside_binary, byte_units, named, one};
use loftspec::decision::DecisionState;
use loftspec::ids::OptionId;
use loftspec::lint::Finding;

/// The text the tests below place where a definition may or may not count.
const DEFINES_MB: &str = "MB = 1,000,000 bytes.";

#[test]
fn a_definition_settles_only_the_units_of_its_own_power() {
    for document in with_text("kB = 1,000 bytes.") {
        assert!(named(&document).contains(&one(FEATURE, "MB")[0]));
    }
    for document in with_text_stating("A page holds 5 kB.", "KB = 1,000 bytes.") {
        assert_eq!(byte_units(&document), Vec::<Finding>::new());
    }
    for document in with_text_stating("A page holds 5 MB, a book 2 GB.", "1 MB = 10^6 bytes.") {
        assert_eq!(named(&document), one(FEATURE, "GB"));
    }
    let both = "KB = 1,024 bytes. MB = 1,000,000 bytes.";
    for document in with_text_stating("A page holds 5 KB, a book 2 MB.", both) {
        assert_eq!(byte_units(&document), Vec::<Finding>::new());
    }
}

#[test]
fn a_count_in_parentheses_after_a_multiplied_unit_defines_nothing() {
    for text in [
        "A 2 MB (2^20 bytes) buffer.",
        "A 64 MB (1,000,000 bytes) cache.",
        "A 1.5 MB (10^6 bytes) page.",
        "Pages of 2 megabytes (2^20 bytes) each.",
    ] {
        for document in with_text(text) {
            assert!(named(&document).contains(&one(FEATURE, "MB")[0]), "{text}");
        }
    }
    for text in [
        "A 1 MB (10^6 bytes) page.",
        "A 1-MB (2^20 bytes) page.",
        "Sizes are in MB (10^6 bytes).",
    ] {
        for document in with_text(text) {
            assert_eq!(byte_units(&document), Vec::<Finding>::new(), "{text}");
        }
    }
}

#[test]
fn an_option_an_accepted_decision_did_not_select_defines_nothing() {
    let unselected: [fn(&mut loftspec::decision::Decision); 3] = [
        |decision| decision.options[1].title = DEFINES_MB.to_owned(),
        |decision| decision.options[1].description = DEFINES_MB.to_owned(),
        |decision| decision.assessments[0].rationale = DEFINES_MB.to_owned(),
    ];
    for edit in unselected {
        let mut document = beside_binary("A page holds 5 MB.");
        edit(decision(&mut document));
        assert_eq!(named(&document), one(FEATURE, "MB"));
    }
}

#[test]
fn an_accepted_decision_defines_in_its_selected_option_and_its_outcome() {
    let in_force: [fn(&mut loftspec::decision::Decision); 4] = [
        |decision| decision.options[0].description = DEFINES_MB.to_owned(),
        |decision| decision.assessments[1].rationale = DEFINES_MB.to_owned(),
        |decision| decision.consequences[0].statement = DEFINES_MB.to_owned(),
        |decision| {
            decision.state = DecisionState::Accepted {
                selected_option: OptionId::new(key(23)),
                outcome: DEFINES_MB.to_owned(),
            };
        },
    ];
    for edit in in_force {
        let mut document = beside_binary("A page holds 5 MB.");
        edit(decision(&mut document));
        assert_eq!(byte_units(&document), Vec::<Finding>::new());
    }
}

#[test]
fn a_decision_that_is_not_accepted_defines_nothing() {
    let selected = OptionId::new(key(23));
    let states = [
        DecisionState::Proposed,
        DecisionState::Rejected {
            rationale: DEFINES_MB.to_owned(),
        },
        DecisionState::Retired {
            selected_option: selected,
            outcome: DEFINES_MB.to_owned(),
            rationale: "Over.".to_owned(),
        },
        superseded(0, 1),
    ];
    for state in states {
        let mut document = beside_binary("A page holds 5 MB.");
        set_state(&mut document, state);
        decision(&mut document).context = DEFINES_MB.to_owned();
        assert_eq!(named(&document), one(FEATURE, "MB"));
    }
}

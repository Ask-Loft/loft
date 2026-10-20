//! The rules of decision history (F-21): `V2-010`, an edit to the meaning of a decision published
//! resolved, and `V2-011`, a change of state its published state does not allow, a resolved
//! decision deleted among them. Both judge from what the last publish recorded in each
//! decision's binding, which is part of the tree.

use crate::common::decisions::{add_decision, decision, decision_id, set_state, superseded};
use crate::common::lint::{findings, keys, of, severity};
use crate::common::publishing::clean_main;
use crate::common::text::{path, text};
use crate::common::{key, label, small_document};
use loftspec::canonical::Digest;
use loftspec::check::check;
use loftspec::codec::{self, Files, LABELS};
use loftspec::decision::{DecisionState, StateKind};
use loftspec::document::{Document, Edge};
use loftspec::ids::{DecisionId, OptionId, RequirementId};
use loftspec::lint::{Finding, RuleId, Severity};
use loftspec::publish::{Made, Publication, PublishedAt, ReadTree, publish, record_decisions};
use loftspec::statement::{EarsForm, Statement};
use loftspec::validate::validate;

/// The small document as a publish leaves it: its decision's binding records its state and its
/// history.
fn published() -> Document {
    let mut document = small_document();
    record_decisions(&mut document);
    document
}

/// The small document published with its decision in `state`.
fn published_in(state: DecisionState) -> Document {
    let mut document = small_document();
    set_state(&mut document, state);
    record_decisions(&mut document);
    document
}

/// The findings of `rule` for `document`.
fn found(document: &Document, rule: RuleId) -> Vec<Finding> {
    of(&findings(document), rule).into_iter().cloned().collect()
}

fn accepted() -> DecisionState {
    DecisionState::Accepted {
        selected_option: OptionId::new(key(23)),
        outcome: "New keys are UUID version 7.".to_owned(),
    }
}

fn retired(outcome: &str) -> DecisionState {
    DecisionState::Retired {
        selected_option: OptionId::new(key(23)),
        outcome: outcome.to_owned(),
        rationale: "No longer needed.".to_owned(),
    }
}

fn rejected() -> DecisionState {
    DecisionState::Rejected {
        rationale: "Not now.".to_owned(),
    }
}

/// The small document's decision superseded, its outcome kept, by a decision added beside it.
fn superseded_by_another(document: &mut Document) {
    let successor = DecisionId::new(key(60));
    document.bindings.bind(successor, label("ADR-2")).unwrap();
    add_decision(
        document,
        40,
        DecisionState::Accepted {
            selected_option: OptionId::new(key(63)),
            outcome: "Replaced.".to_owned(),
        },
    );
    let mut state = superseded(0, 60);
    if let DecisionState::Superseded { outcome, .. } = &mut state {
        "New keys are UUID version 7.".clone_into(outcome);
    }
    set_state(document, state);
}

#[test]
fn an_edit_to_a_decision_in_the_state_it_was_published_in_is_one_v2_010() {
    for state in [accepted(), rejected()] {
        let mut document = published_in(state);
        decision(&mut document).context.push_str(" Edited.");
        let found = found(&document, RuleId::V2010);
        assert_eq!(found.len(), 1);
        assert_eq!(keys(&found[0]), [decision_id().key()]);
    }
}

#[test]
fn a_decision_whose_history_hash_is_the_published_one_is_not_v2_010() {
    assert_eq!(found(&published(), RuleId::V2010), []);
    assert_eq!(found(&published_in(rejected()), RuleId::V2010), []);
}

#[test]
fn a_decision_retired_or_superseded_after_an_edit_is_one_v2_010() {
    let mut document = published();
    set_state(&mut document, retired("Another outcome."));
    assert_eq!(found(&document, RuleId::V2010).len(), 1);
    let mut document = published();
    superseded_by_another(&mut document);
    decision(&mut document).title.push_str(" (edited)");
    assert_eq!(found(&document, RuleId::V2010).len(), 1);
}

#[test]
fn a_decision_retired_or_superseded_with_nothing_else_changed_is_not_v2_010() {
    let mut document = published();
    set_state(&mut document, retired("New keys are UUID version 7."));
    assert_eq!(found(&document, RuleId::V2010), []);
    let mut document = published();
    superseded_by_another(&mut document);
    assert_eq!(found(&document, RuleId::V2010), []);
}

#[test]
fn an_edit_to_a_node_a_decision_weighs_is_not_v2_010() {
    let mut document = published();
    document.products[0].feature_groups[0].features[0].requirements[0].statement =
        Statement::Form(EarsForm::Ubiquitous {
            system: "The writer".to_owned(),
            shall: "write a key on each node.".to_owned(),
        });
    assert_eq!(found(&document, RuleId::V2010), []);
}

#[test]
fn a_change_to_a_decision_s_subjects_or_mentions_alone_is_not_v2_010() {
    let mut document = published();
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Subject { .. }));
    document.edges.push(Edge::Mentions {
        source: decision_id().into(),
        target: RequirementId::new(key(4)).into(),
    });
    assert_eq!(found(&document, RuleId::V2010), []);
}

#[test]
fn a_decision_without_a_published_history_or_a_binding_is_not_v2_010() {
    let mut document = small_document();
    let bindings = &mut document.bindings;
    bindings
        .record_published(decision_id(), Some(StateKind::Accepted), None)
        .unwrap();
    decision(&mut document).context.push_str(" Edited.");
    assert_eq!(found(&document, RuleId::V2010), []);
    let mut document = published();
    document.bindings.unbind(decision_id()).unwrap();
    decision(&mut document).context.push_str(" Edited.");
    assert_eq!(found(&document, RuleId::V2010), []);
}

#[test]
fn a_v2_010_finding_names_the_decision_and_its_published_state() {
    let mut document = published();
    decision(&mut document).context.push_str(" Edited.");
    let found = found(&document, RuleId::V2010);
    assert_eq!(keys(&found[0]), [decision_id().key()]);
    assert!(
        found[0].message.contains("`accepted`"),
        "{}",
        found[0].message
    );
}

#[test]
fn v2_010_and_v2_011_are_warnings_by_default() {
    assert_eq!(severity(RuleId::V2010), Severity::Warning);
    assert_eq!(severity(RuleId::V2011), Severity::Warning);
}

/// The `V2-011` findings of the small document published in `from` and now in `to`.
fn moved(from: DecisionState, to: DecisionState) -> Vec<Finding> {
    let mut document = small_document();
    superseded_by_another(&mut document);
    let successor = decision(&mut document).state.clone();
    let resolve = |state: DecisionState| match state {
        DecisionState::Superseded { .. } => successor.clone(),
        state => state,
    };
    set_state(&mut document, resolve(from));
    record_decisions(&mut document);
    set_state(&mut document, resolve(to));
    found(&document, RuleId::V2011)
}

fn proposed() -> DecisionState {
    DecisionState::Proposed
}

/// A superseded state; [`moved`] gives it its successor.
fn superseded_state() -> DecisionState {
    superseded(0, 60)
}

#[test]
fn an_accepted_decision_now_superseded_or_retired_is_not_v2_011() {
    assert_eq!(moved(accepted(), superseded_state()), []);
    assert_eq!(moved(accepted(), retired("Kept.")), []);
}

#[test]
fn a_proposed_decision_now_accepted_or_rejected_is_not_v2_011() {
    assert_eq!(moved(proposed(), accepted()), []);
    assert_eq!(moved(proposed(), rejected()), []);
}

#[test]
fn a_proposed_decision_now_superseded_or_retired_is_one_v2_011() {
    assert_eq!(moved(proposed(), superseded_state()).len(), 1);
    assert_eq!(moved(proposed(), retired("Kept.")).len(), 1);
}

#[test]
fn an_accepted_decision_now_proposed_or_rejected_is_one_v2_011() {
    assert_eq!(moved(accepted(), proposed()).len(), 1);
    assert_eq!(moved(accepted(), rejected()).len(), 1);
}

fn every_state() -> [DecisionState; 5] {
    [
        proposed(),
        accepted(),
        rejected(),
        superseded_state(),
        retired("Kept."),
    ]
}

#[test]
fn a_rejected_superseded_or_retired_decision_whose_state_has_another_kind_is_one_v2_011() {
    for from in [rejected(), superseded_state(), retired("Kept.")] {
        for to in every_state() {
            let expected = usize::from(to.kind() != from.kind());
            let found = moved(from.clone(), to.clone());
            assert_eq!(found.len(), expected, "{from:?} to {to:?}");
        }
    }
}

#[test]
fn a_decision_in_the_kind_of_state_it_was_published_in_is_not_v2_011() {
    for state in every_state() {
        assert_eq!(moved(state.clone(), state), []);
    }
    assert_eq!(moved(retired("Kept."), retired("Another.")), []);
}

#[test]
fn a_decision_whose_binding_records_no_published_state_is_not_v2_011() {
    let mut document = small_document();
    set_state(&mut document, rejected());
    assert_eq!(found(&document, RuleId::V2011), []);
}

#[test]
fn a_v2_011_finding_names_the_decision_its_published_state_and_its_state_now() {
    let found = moved(accepted(), rejected());
    assert_eq!(keys(&found[0]), [decision_id().key()]);
    let message = &found[0].message;
    assert!(message.contains("`accepted`"), "{message}");
    assert!(message.contains("`rejected`"), "{message}");
}

/// The small document with the departed binding of a decision `ADR-5`, published in `state`.
fn with_departed(state: StateKind) -> Document {
    let mut document = published();
    let gone = DecisionId::new(key(90));
    let history = "ab".repeat(32).parse::<Digest>().unwrap();
    let bindings = &mut document.bindings;
    bindings.bind(gone, label("ADR-5")).unwrap();
    let history = (state != StateKind::Proposed).then_some(history);
    bindings
        .record_published(gone, Some(state), history)
        .unwrap();
    bindings.depart(gone).unwrap();
    document
}

#[test]
fn a_departed_decision_published_resolved_is_one_v2_011_naming_its_key_and_label() {
    use StateKind::{Accepted, Rejected, Retired, Superseded};
    for state in [Accepted, Rejected, Superseded, Retired] {
        let found = found(&with_departed(state), RuleId::V2011);
        assert_eq!(found.len(), 1, "{state:?}");
        assert_eq!(keys(&found[0]), [key(90)]);
        assert!(found[0].message.contains("ADR-5"), "{}", found[0].message);
    }
}

#[test]
fn a_departed_decision_published_proposed_is_not_v2_011() {
    assert_eq!(
        found(&with_departed(StateKind::Proposed), RuleId::V2011),
        []
    );
}

#[test]
fn the_finding_for_a_departed_decision_says_retiring_it_is_the_way_to_end_it() {
    let found = found(&with_departed(StateKind::Accepted), RuleId::V2011);
    assert!(found[0].message.contains("retir"), "{}", found[0].message);
}

/// The small document published, with its decision's binding still marked departed, as the
/// formatter leaves the binding of a decision that came back.
fn came_back() -> Document {
    let mut document = published();
    document.bindings.depart(decision_id()).unwrap();
    document
}

#[test]
fn a_decision_back_in_the_tree_is_judged_as_present_though_its_binding_is_departed() {
    assert_eq!(found(&came_back(), RuleId::V2011), []);
    let mut edited = came_back();
    decision(&mut edited).context.push_str(" Edited.");
    assert_eq!(found(&edited, RuleId::V2010).len(), 1);
    let mut moved = came_back();
    set_state(&mut moved, rejected());
    let found = found(&moved, RuleId::V2011);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].message.contains("`rejected`"),
        "{}",
        found[0].message
    );
}

#[test]
fn the_validator_reports_no_error_for_a_decision_v2_010_or_v2_011_reports() {
    let mut edited = published();
    decision(&mut edited).context.push_str(" Edited.");
    let mut moved = published();
    set_state(&mut moved, rejected());
    let departed = with_departed(StateKind::Accepted);
    for (document, rule) in [
        (edited, RuleId::V2010),
        (moved, RuleId::V2011),
        (departed, RuleId::V2011),
    ] {
        assert_eq!(found(&document, rule).len(), 1, "{rule}");
        assert_eq!(validate(&document), [], "{rule}");
    }
}

/// The files of the small document published, its decision then edited, and the departed
/// binding of a resolved decision beside it.
fn edited_and_departed() -> Files {
    let mut document = with_departed(StateKind::Accepted);
    decision(&mut document).context.push_str(" Edited.");
    codec::write(&document)
}

/// The rules of decision history among the findings of checking `files`.
fn history_rules(files: &Files) -> Vec<RuleId> {
    let checked = check(files);
    assert!(!checked.has_errors(), "{checked:?}");
    let rules = checked.findings().iter().map(|finding| finding.rule);
    let mut rules: Vec<RuleId> = rules
        .filter(|rule| matches!(rule, RuleId::V2010 | RuleId::V2011))
        .collect();
    rules.sort_unstable();
    rules
}

#[test]
fn decision_history_is_judged_from_the_bindings_file_of_the_tree_alone() {
    let files = edited_and_departed();
    assert_eq!(history_rules(&files), [RuleId::V2010, RuleId::V2011]);
    let labels = text(&files, LABELS);
    let unrecorded: String = labels
        .split_inclusive('\n')
        .filter(|line| !line.starts_with("    published"))
        .collect();
    assert_ne!(unrecorded, labels);
    let mut files = files;
    files.insert(path(LABELS), unrecorded.into_bytes());
    assert_eq!(history_rules(&files), []);
}

#[test]
fn a_tree_that_v2_010_and_v2_011_report_is_published() {
    let files = edited_and_departed();
    assert_eq!(history_rules(&files).len(), 2);
    let repo = clean_main();
    let at = PublishedAt::from_unix_seconds(1_000).unwrap();
    let publication = publish(
        ReadTree::read(&files).unwrap(),
        Some(&repo),
        None,
        at,
        Made::NotAsRemote,
    );
    assert!(
        matches!(publication, Ok(Publication::Published { .. })),
        "{publication:?}"
    );
}

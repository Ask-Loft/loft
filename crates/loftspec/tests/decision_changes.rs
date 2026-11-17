//! The decisions one document added, changed and removed against its base, with the subjects
//! each gained and lost (the `loft status` and See a spec change Features): a former label and
//! the node it stood for are one node, in a decision's subjects, drivers and consequences alike.

use std::collections::BTreeSet;

use crate::common::{key, label, proposed_decision, small_document};
use loftspec::decision::{DriverPayload, NodeRef};
use loftspec::diff::{DecisionChanges, Subject, decision_changes};
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::ids::{ConstraintId, DecisionId, RequirementId};

/// The small document's decision, ADR-1.
fn adr() -> DecisionId {
    DecisionId::new(key(20))
}

/// Requirement RQ-2, which ADR-1 names as its outcome, in a driver and in a consequence.
fn requirement() -> NodeRef {
    RequirementId::new(key(4)).into()
}

/// `document` with each reference of ADR-1 to RQ-2 written as `to`: its outcome edge, its driver
/// and its consequence.
fn renamed(mut document: Document, to: NodeRef) -> Document {
    for edge in &mut document.edges {
        if let Edge::Subject { subject, .. } = edge
            && *subject == requirement()
        {
            *subject = to;
        }
    }
    let decision = &mut document.decisions[0];
    for driver in &mut decision.drivers {
        if let DriverPayload::Subject { subject } = &mut driver.payload {
            *subject = to;
        }
    }
    for consequence in &mut decision.consequences {
        if consequence.subject.is_some() {
            consequence.subject = Some(to);
        }
    }
    document
}

fn subjects(subjects: &[Subject]) -> BTreeSet<Subject> {
    subjects.iter().copied().collect()
}

#[test]
fn a_document_compared_with_itself_changes_no_decision() {
    let changes = decision_changes(&small_document(), &small_document());
    assert_eq!(changes, DecisionChanges::default());
    assert!(changes.is_empty());
}

#[test]
fn a_decision_only_one_version_holds_is_added_or_removed_with_its_subjects() {
    let mut document = small_document();
    document
        .decisions
        .push(proposed_decision(key(40), "a-proposal"));
    let added = DecisionId::new(key(40));
    document.edges.push(Edge::Subject {
        decision: added,
        subject: requirement(),
        role: SubjectRole::Affected,
    });
    let changes = decision_changes(&small_document(), &document);
    assert_eq!(changes.added, [added]);
    assert_eq!(
        changes.subjects[&added].gained,
        subjects(&[(requirement(), SubjectRole::Affected)])
    );
    let back = decision_changes(&document, &small_document());
    assert_eq!(back.removed, [added]);
    assert_eq!(back.changed, []);
    assert_eq!(
        back.subjects[&added].lost,
        subjects(&[(requirement(), SubjectRole::Affected)])
    );
}

#[test]
fn a_decision_whose_fields_differ_is_changed_without_a_subject_change() {
    let mut document = small_document();
    document.decisions[0].title.push_str(", time-ordered");
    let changes = decision_changes(&small_document(), &document);
    assert_eq!(changes.changed, [adr()]);
    assert!(changes.subjects.is_empty(), "{changes:?}");
}

#[test]
fn a_decision_whose_only_change_is_a_subject_gained_or_lost_is_changed() {
    let mut document = small_document();
    let constraint: NodeRef = ConstraintId::new(key(7)).into();
    document.edges.push(Edge::Subject {
        decision: adr(),
        subject: constraint,
        role: SubjectRole::Affected,
    });
    document.edges.retain(|edge| {
        !matches!(
            edge,
            Edge::Subject {
                role: SubjectRole::Outcome,
                ..
            }
        )
    });
    let changes = decision_changes(&small_document(), &document);
    assert_eq!(changes.changed, [adr()]);
    let change = &changes.subjects[&adr()];
    assert_eq!(
        change.gained,
        subjects(&[(constraint, SubjectRole::Affected)])
    );
    assert_eq!(
        change.lost,
        subjects(&[(requirement(), SubjectRole::Outcome)])
    );
}

#[test]
fn a_decision_that_names_a_departed_node_by_its_former_label_is_unchanged() {
    let mut document = renamed(small_document(), NodeRef::Former(label("RQ-2")));
    document
        .bindings
        .depart(RequirementId::new(key(4)))
        .unwrap();
    assert_eq!(
        decision_changes(&small_document(), &document),
        DecisionChanges::default()
    );
    assert_eq!(
        decision_changes(&document, &small_document()),
        DecisionChanges::default()
    );
}

#[test]
fn a_former_label_that_a_live_binding_still_gives_names_its_node() {
    let document = renamed(small_document(), NodeRef::Former(label("RQ-2")));
    assert!(decision_changes(&small_document(), &document).is_empty());
}

#[test]
fn a_former_label_no_binding_gives_is_a_subject_of_its_own() {
    let unknown = NodeRef::Former(label("RQ-77"));
    let document = renamed(small_document(), unknown);
    let changes = decision_changes(&small_document(), &document);
    assert_eq!(changes.changed, [adr()]);
    let change = &changes.subjects[&adr()];
    assert_eq!(change.gained, subjects(&[(unknown, SubjectRole::Outcome)]));
    assert_eq!(
        change.lost,
        subjects(&[(requirement(), SubjectRole::Outcome)])
    );
}

#[test]
fn a_decision_whose_only_change_is_a_mentions_entry_is_changed() {
    let mut document = small_document();
    document.edges.push(Edge::Mentions {
        source: adr().into(),
        target: RequirementId::new(key(4)).into(),
    });
    let changes = decision_changes(&small_document(), &document);
    assert_eq!(changes.changed, [adr()]);
    assert!(changes.subjects.is_empty(), "{changes:?}");
    assert_eq!(
        decision_changes(&document, &small_document()).changed,
        [adr()]
    );
}

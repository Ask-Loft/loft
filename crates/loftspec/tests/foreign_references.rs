//! References into another document: a record of the other document's handle and a node's key,
//! resolved against the copies under that handle, in `provides`, `requires`, `mentions` and a
//! subject's `node` only, and never a decision's outcome.

use crate::common::copies::{HANDLE, SHELF, requiring, requiring_two};
use crate::common::fields::{assert_required, fields};
use crate::common::interfaces::{INTERFACE, MODEL, provides, requires};
use crate::common::key;
use crate::common::text::{path, text};
use loftspec::codec::{self, Files, Problem, ReadError};
use loftspec::decision::NodeRef;
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::identity::Key;
use loftspec::ids::{CriterionId, DecisionId, ElementId, NodeId, RequirementId};
use loftspec::validate::{ValidationError, validate};

const COMPONENT: &str = "components/document-model.yaml";
const FEATURE: &str = "products/example-product/identity/keys.yaml";
const DECISION: &str = "decisions/keys-are-uuids.yaml";

/// The small document's decision, keyed `0x14`.
fn decision() -> DecisionId {
    DecisionId::new(key(0x14))
}

/// The foreign Requirement and criterion of the copy.
fn promise() -> NodeId {
    RequirementId::new(key(INTERFACE + 1)).into()
}

fn answer() -> NodeId {
    CriterionId::new(key(INTERFACE + 2)).into()
}

/// The requiring document with a foreign edge of each kind: its root Component provides the
/// copied Interface, its criterion mentions the copied criterion, and its decision names the
/// copied Requirement as affected.
fn crossing() -> Document {
    let mut document = requiring();
    document.edges.extend([
        provides(MODEL, INTERFACE),
        Edge::Mentions {
            source: ElementId::Node(CriterionId::new(key(5)).into()),
            target: ElementId::Node(answer()),
        },
        Edge::Subject {
            decision: decision(),
            subject: NodeRef::Held(promise()),
            role: SubjectRole::Affected,
        },
    ]);
    document
}

/// The files with the entry `old` of the list field `name` of the file `at` written as a
/// reference to `new` in the copies under `handle`.
fn crossed(files: &Files, at: &str, name: &str, old: Key, handle: &str, new: Key) -> Files {
    let source = text(files, at);
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let field = (lines.iter())
        .position(|line| line.trim_start() == format!("{name}:"))
        .unwrap_or_else(|| panic!("no {name} in {at}"));
    let entry = (field..lines.len())
        .find(|&line| lines[line].trim_start() == format!("- {old}"))
        .unwrap_or_else(|| panic!("no {old} under {name} in {at}"));
    let indent = lines[entry].len() - lines[entry].trim_start().len();
    let (dash, pad) = (" ".repeat(indent), " ".repeat(indent + 2));
    lines[entry] = format!("{dash}- document: {handle}\n{pad}key: {new}");
    let mut changed = files.clone();
    changed.insert(path(at), format!("{}\n", lines.join("\n")).into_bytes());
    changed
}

fn read_errors(files: &Files) -> Vec<ReadError> {
    codec::read(files).expect_err("refused")
}

fn message(errors: &[ReadError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_foreign_reference_is_a_record_of_document_and_key_and_requires_both() {
    let files = codec::write(&crossing());
    let route = &["provides", "0"];
    assert_eq!(fields(&files, COMPONENT, route), ["document", "key"]);
    for field in ["document", "key"] {
        assert_required(&files, COMPONENT, route, field);
    }
}

#[test]
fn a_foreign_references_document_holds_a_handle_of_documents() {
    let files = codec::write(&crossing());
    let component = text(&files, COMPONENT);
    let reference = format!(
        "provides:\n  - document: {HANDLE}\n    key: {}\n",
        key(INTERFACE)
    );
    assert!(component.contains(&reference), "{component}");
}

#[test]
fn a_foreign_reference_stands_in_provides_requires_mentions_and_a_subjects_node() {
    let mut document = crossing();
    document.edges.push(requires(MODEL, SHELF));
    let mut document_two = requiring_two();
    document_two.edges = document.edges.clone();
    let back = codec::read(&codec::write(&document_two)).unwrap();
    for edge in &document_two.edges {
        assert!(back.edges.contains(edge), "{edge:?}");
    }
    assert_eq!(validate(&back), vec![]);
}

#[test]
fn foreign_references_follow_the_documents_own_keys_by_handle_then_key() {
    let mut document = requiring_two();
    let source = ElementId::Node(CriterionId::new(key(5)).into());
    let targets = [
        answer(),
        NodeId::from(RequirementId::new(key(4))),
        RequirementId::new(key(SHELF + 1)).into(),
        promise(),
    ];
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Mentions { .. }));
    document.edges.extend(targets.map(|target| Edge::Mentions {
        source,
        target: ElementId::Node(target),
    }));
    let feature = text(&codec::write(&document), FEATURE);
    let written = feature.split("mentions:\n").nth(1).unwrap();
    let order = [
        format!("- {}", key(4)),
        format!(
            "- document: catalog\n{}key: {}",
            " ".repeat(written.find("- ").unwrap() + 2),
            key(SHELF + 1)
        ),
        format!("key: {}", key(INTERFACE + 1)),
        format!("key: {}", key(INTERFACE + 2)),
    ];
    let places: Vec<usize> = order
        .iter()
        .map(|each| {
            written
                .find(each.as_str())
                .unwrap_or_else(|| panic!("{each} in {written}"))
        })
        .collect();
    assert!(places.windows(2).all(|pair| pair[0] < pair[1]), "{written}");
}

#[test]
fn a_foreign_reference_in_realizes_or_scoped_to_is_refused_naming_the_field() {
    let files = codec::write(&crossing());
    let wrong = crossed(
        &files,
        FEATURE,
        "realizes",
        key(MODEL),
        HANDLE,
        key(INTERFACE),
    );
    let errors = read_errors(&wrong);
    assert_eq!(errors[0].problem, Problem::NotCrossing("realizes"));
    assert!(
        errors[0].to_string().contains("`realizes`"),
        "{}",
        errors[0]
    );
    let wrong = crossed(
        &files,
        COMPONENT,
        "scopedTo",
        key(1),
        HANDLE,
        key(INTERFACE),
    );
    assert_eq!(
        read_errors(&wrong)[0].problem,
        Problem::NotCrossing("scopedTo")
    );
}

/// The decision file with its `subject: <key>` line numbered `line` written as a reference to
/// the copied Requirement.
fn subject_crossed(files: &Files, line: usize) -> Files {
    let source = text(files, DECISION);
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let indent = lines[line].len() - lines[line].trim_start().len();
    let pad = " ".repeat(indent + 2);
    lines[line] = format!(
        "{}subject:\n{pad}document: {HANDLE}\n{pad}key: {}",
        " ".repeat(indent),
        key(INTERFACE + 1)
    );
    let mut changed = files.clone();
    changed.insert(
        path(DECISION),
        format!("{}\n", lines.join("\n")).into_bytes(),
    );
    changed
}

#[test]
fn a_foreign_reference_in_a_successor_or_a_drivers_or_consequences_subject_is_refused() {
    let files = codec::write(&crossing());
    let decision = text(&files, DECISION);
    let subjects: Vec<usize> = (decision.lines().enumerate())
        .filter(|(_, line)| line.trim_start().starts_with("subject: "))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(
        subjects.len(),
        2,
        "a driver's and a consequence's: {decision}"
    );
    for line in subjects {
        let errors = read_errors(&subject_crossed(&files, line));
        assert_eq!(errors[0].problem, Problem::NotCrossing("subject"));
    }
    let state = decision.replace(
        "  kind: accepted\n",
        &format!(
            "  kind: accepted\n  successor:\n    document: {HANDLE}\n    key: {}\n",
            key(INTERFACE)
        ),
    );
    assert_ne!(state, decision);
    let mut wrong = files;
    wrong.insert(path(DECISION), state.into_bytes());
    assert_eq!(
        read_errors(&wrong)[0].problem,
        Problem::NotCrossing("successor")
    );
}

#[test]
fn a_foreign_reference_under_a_handle_documents_does_not_list_is_refused() {
    let files = codec::write(&crossing());
    let component = text(&files, COMPONENT).replace("document: format", "document: store");
    let mut wrong = files;
    wrong.insert(path(COMPONENT), component.into_bytes());
    let errors = message(&read_errors(&wrong));
    assert!(
        errors.contains(&format!("store:{}", key(INTERFACE))),
        "{errors}"
    );
    assert!(errors.contains("does not list"), "{errors}");
}

#[test]
fn a_key_the_other_document_holds_outside_every_copy_is_refused() {
    let files = codec::write(&crossing());
    let outside = key(MODEL);
    let component = text(&files, COMPONENT).replace(
        &format!("key: {}\n", key(INTERFACE)),
        &format!("key: {outside}\n"),
    );
    let mut wrong = files;
    wrong.insert(path(COMPONENT), component.into_bytes());
    let errors = message(&read_errors(&wrong));
    assert!(errors.contains(&format!("{HANDLE}:{outside}")), "{errors}");
    assert!(errors.contains("no copy under the handle"), "{errors}");
}

#[test]
fn a_key_that_a_copy_under_another_handle_holds_is_refused() {
    let mut document = requiring_two();
    document.edges.push(requires(MODEL, SHELF));
    let files = codec::write(&document);
    let component = text(&files, COMPONENT).replace("document: catalog", "document: format");
    let mut wrong = files;
    wrong.insert(path(COMPONENT), component.into_bytes());
    let errors = message(&read_errors(&wrong));
    assert!(
        errors.contains(&format!("format:{}", key(SHELF))),
        "{errors}"
    );
}

#[test]
fn a_reference_whose_copy_holds_its_key_raises_no_error() {
    let back = codec::read(&codec::write(&crossing())).unwrap();
    assert_eq!(validate(&back), vec![]);
}

#[test]
fn a_foreign_outcome_subject_is_an_error_naming_the_decision_and_the_node() {
    let mut document = crossing();
    document.edges.push(Edge::Subject {
        decision: decision(),
        subject: NodeRef::Held(answer()),
        role: SubjectRole::Outcome,
    });
    let errors = validate(&document);
    let expected = ValidationError::ForeignOutcome {
        decision: decision(),
        node: answer(),
    };
    assert_eq!(errors, vec![expected.clone()]);
    assert_eq!(expected.keys(), vec![decision().key(), answer().key()]);
    assert!(
        expected
            .to_string()
            .contains("a decision's outcome is a node of its own document")
    );
}

#[test]
fn a_foreign_affected_subject_raises_no_error() {
    assert_eq!(validate(&crossing()), vec![]);
}

#[test]
fn an_unresolved_foreign_reference_is_an_error_naming_the_element_that_makes_it() {
    let files = codec::write(&crossing());
    let source = format!("component {}", key(MODEL));
    let unlisted = text(&files, COMPONENT).replace("document: format", "document: store");
    let outside = text(&files, COMPONENT).replace(
        &format!("key: {}\n", key(INTERFACE)),
        &format!("key: {}\n", key(MODEL)),
    );
    let cases = [
        (unlisted, format!("store:{}", key(INTERFACE))),
        (outside, format!("{HANDLE}:{}", key(MODEL))),
    ];
    for (component, reference) in cases {
        let mut wrong = files.clone();
        wrong.insert(path(COMPONENT), component.into_bytes());
        let errors = message(&read_errors(&wrong));
        assert!(errors.contains(&source), "{errors}");
        assert!(errors.contains(&reference), "{errors}");
    }
}

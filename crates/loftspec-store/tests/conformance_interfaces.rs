//! The document stores' scenario continued with an Interface: a root Component that provides
//! it, another that requires it, a change to its promise and its withdrawal. After each step the
//! memory store and the file store agree, as the harness of `conformance` checks.

use crate::conformance::{
    Pair, created, criterion, key, linked, paths, tagged, walk, with_requirement,
};
use loftspec::document::{Document, Edge};
use loftspec::ids::{ComponentId, InterfaceId, RequirementId};
use loftspec::slug::Slug;
use loftspec::statement::{EarsForm, Statement};
use loftspec::tree::{Component, Details, Interface, Requirement, RootComponent};

const LEDGER: u32 = 30;
const DESK: u32 = 31;
const LOANS: u32 = 40;

/// After linking: "Loan records", a promise the Ledger provides and a Front desk requires.
fn promised() -> Document {
    let mut document = linked();
    document.components.push(RootComponent {
        slug: Slug::derive("Front desk", "component"),
        component: Component {
            id: Some(ComponentId::new(key(DESK))),
            name: "Front desk".to_owned(),
            details: Details::default(),
            boundary: None,
            components: Vec::new(),
            constraints: Vec::new(),
            quality_attributes: Vec::new(),
        },
    });
    document.interfaces.push(Interface {
        id: Some(InterfaceId::new(key(LOANS))),
        slug: Slug::derive("Loan records", "interface"),
        name: "Loan records".to_owned(),
        details: Details::with_body("What a desk may ask the ledger."),
        requirements: vec![Requirement {
            id: Some(RequirementId::new(key(LOANS + 1))),
            statement: Statement::Form(EarsForm::EventDriven {
                when: "a desk asks for a reader's loans".to_owned(),
                system: "The ledger".to_owned(),
                shall: "list the reader's loans.".to_owned(),
            }),
            details: Details::default(),
            criteria: vec![criterion(LOANS + 2, "Each open loan is listed once.")],
        }],
        constraints: Vec::new(),
        quality_attributes: Vec::new(),
    });
    let interface = InterfaceId::new(key(LOANS));
    document.edges.extend([
        Edge::Provides {
            component: ComponentId::new(key(LEDGER)),
            interface,
        },
        Edge::Requires {
            component: ComponentId::new(key(DESK)),
            interface,
        },
    ]);
    document
}

/// Then the promise's body changes.
fn promise_changed() -> Document {
    let mut document = promised();
    document.interfaces[0].details.body = Some("What a desk may ask the ledger, and how.".into());
    document
}

/// Or the promise is withdrawn, and the edges to it with it.
fn promise_withdrawn() -> Document {
    let mut document = promised();
    document.interfaces.clear();
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Provides { .. } | Edge::Requires { .. }));
    document
}

const BEFORE: [fn() -> Document; 4] = [created, with_requirement, tagged, linked];

fn after(steps: &[fn() -> Document]) -> Vec<fn() -> Document> {
    BEFORE.iter().chain(steps).copied().collect()
}

#[test]
fn adding_an_interface_writes_its_file_and_the_two_components_that_link_to_it() {
    let saved = walk(&after(&[promised]));
    assert_eq!(
        saved.written,
        paths(&[
            "components/front-desk.yaml",
            "components/ledger.yaml",
            "interfaces/loan-records.yaml",
        ])
    );
    assert_eq!(saved.removed, Vec::new());
}

#[test]
fn changing_an_interface_writes_only_its_file() {
    let saved = walk(&after(&[promised, promise_changed]));
    assert_eq!(saved.written, paths(&["interfaces/loan-records.yaml"]));
}

#[test]
fn withdrawing_an_interface_removes_its_file_and_its_directory() {
    let mut pair = Pair::new();
    for step in after(&[promised, promise_withdrawn]) {
        pair.step(&step());
    }
    assert!(!pair.scratch.0.join("spec/interfaces").exists());
    let saved = walk(&after(&[promised, promise_withdrawn]));
    assert_eq!(
        saved.written,
        paths(&["components/front-desk.yaml", "components/ledger.yaml"])
    );
    assert_eq!(saved.removed, paths(&["interfaces/loan-records.yaml"]));
}

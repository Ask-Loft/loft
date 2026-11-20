//! The document stores' scenario continued with another document: the Ledger provides an
//! Interface a copy holds, the pin moves, and the copy goes. After each step the memory store and
//! the file store agree, as the harness of `conformance` checks.

use crate::conformance::{
    Pair, created, criterion, key, linked, paths, tagged, walk, with_requirement,
};
use loftspec::canonical::Digest;
use loftspec::codec::{self, copy_of};
use loftspec::copy::Copy;
use loftspec::document::{Document, Edge, OtherDocument};
use loftspec::ids::{ComponentId, DocumentId, InterfaceId, RequirementId};
use loftspec::publish::allocate;
use loftspec::slug::Slug;
use loftspec::statement::{EarsForm, Statement};
use loftspec::tree::{Details, Interface, Requirement};
use loftspec_store::Saved;

const LEDGER: u32 = 30;
const OTHER: u32 = 50;
const AUDIT: u32 = 60;
const HANDLE: &str = "audit";

/// The other document: one Interface, "Audit trail", every node labeled.
fn other() -> Document {
    let mut document = Document::empty(DocumentId::new(key(OTHER)));
    document.interfaces.push(Interface {
        id: Some(InterfaceId::new(key(AUDIT))),
        slug: Slug::derive("Audit trail", "interface"),
        name: "Audit trail".to_owned(),
        details: Details::with_body("What an auditor may read of the ledger."),
        requirements: vec![Requirement {
            id: Some(RequirementId::new(key(AUDIT + 1))),
            statement: Statement::Form(EarsForm::EventDriven {
                when: "an auditor asks for a day's entries".to_owned(),
                system: "The ledger".to_owned(),
                shall: "list the day's entries.".to_owned(),
            }),
            details: Details::default(),
            criteria: vec![criterion(AUDIT + 2, "Each entry is listed once.")],
        }],
        constraints: Vec::new(),
        quality_attributes: Vec::new(),
    });
    allocate(&mut document);
    document
}

/// The copy of the other document's Interface at `sequence`.
fn copy_at(sequence: u64) -> Copy {
    let root: Digest = format!("{sequence:02x}").repeat(32).parse().unwrap();
    let handle = Slug::parse(HANDLE).unwrap();
    copy_of(&handle, &codec::write(&other()), key(AUDIT), sequence, root).unwrap()
}

/// After linking: the document is connected, lists the other document, holds a copy of its
/// Interface at publish sequence 2, and the Ledger provides it.
fn pinned() -> Document {
    let mut document = linked();
    document.connected = true;
    document.remote = Some("https://remote.example/documents/library".to_owned());
    document.documents.push(OtherDocument {
        handle: Slug::parse(HANDLE).unwrap(),
        key: DocumentId::new(key(OTHER)),
        remote: "https://remote.example/documents/audit".to_owned(),
    });
    document.copies.push(copy_at(2));
    document.edges.push(Edge::Provides {
        component: ComponentId::new(key(LEDGER)),
        interface: InterfaceId::new(key(AUDIT)),
    });
    document
}

/// Then the pin moves to publish sequence 3.
fn moved() -> Document {
    let mut document = pinned();
    document.copies = vec![copy_at(3)];
    document
}

/// Or the copy goes, with the edge to it and the other document's entry.
fn removed() -> Document {
    let mut document = pinned();
    document.copies.clear();
    document.documents.clear();
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Provides { .. }));
    document
}

const BEFORE: [fn() -> Document; 4] = [created, with_requirement, tagged, linked];

fn after(steps: &[fn() -> Document]) -> Vec<fn() -> Document> {
    BEFORE.iter().chain(steps).copied().collect()
}

#[test]
fn pinning_a_copy_writes_the_manifest_the_copy_and_the_component_that_provides_it() {
    let saved = walk(&after(&[pinned]));
    assert_eq!(
        saved.written,
        paths(&[
            "components/ledger.yaml",
            "documents/audit/audit-trail.yaml",
            "loftspec.yaml"
        ])
    );
    assert_eq!(saved.removed, Vec::new());
}

#[test]
fn moving_a_pin_writes_only_the_copy() {
    let saved = walk(&after(&[pinned, moved]));
    assert_eq!(saved.written, paths(&["documents/audit/audit-trail.yaml"]));
}

#[test]
fn removing_a_copy_removes_its_file_and_its_directory() {
    let mut pair = Pair::new();
    let mut saved = Saved::default();
    for step in after(&[pinned, removed]) {
        saved = pair.step(&step());
    }
    assert!(!pair.scratch.0.join("spec/documents").exists());
    assert_eq!(saved.removed, paths(&["documents/audit/audit-trail.yaml"]));
    assert_eq!(
        saved.written,
        paths(&["components/ledger.yaml", "loftspec.yaml"])
    );
}

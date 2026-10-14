//! The reconciliation findings and accepting a link (the Feature "Reconciliation findings"),
//! over the small document: its nodes in scope are criterion 5, CON-2 (9) and CON-1 (7); QA-1
//! (8) and QA-2 (10) are sampled, and every other node is of another kind.

use std::collections::HashMap;

use crate::common::{key, label, small_document};
use loftspec::attributes::VerificationFate;
use loftspec::document::Document;
use loftspec::hash::{DocumentHashes, hash_document};
use loftspec::identity::Key;
use loftspec::ids::{CriterionId, NodeId};
use loftspec::lint::Severity;
use loftspec::trace::{
    Evidence, Link, Reconciled, Refusal, Test, TestFile, TestFiles, Trace, TraceRule, accept,
    accept_stale, reconcile,
};

/// Test files held in memory; a path not held is missing.
#[derive(Default)]
struct Files(HashMap<String, TestFile>);

impl TestFiles for Files {
    fn read(&self, path: &str) -> TestFile {
        self.0.get(path).cloned().unwrap_or(TestFile::Missing)
    }
}

/// One file, `tests/a.rs`, holding `text`.
fn files(text: &str) -> Files {
    Files(HashMap::from([(
        "tests/a.rs".to_owned(),
        TestFile::Text(text.to_owned()),
    )]))
}

fn hashes(document: &Document) -> DocumentHashes {
    hash_document(document).unwrap()
}

/// The content hash of node `n` of the small document.
fn hash_of(document: &Document, id: NodeId) -> loftspec::canonical::Digest {
    hashes(document).nodes[&id]
}

fn criterion_id() -> NodeId {
    CriterionId::new(key(5)).into()
}

/// A link to node `n`, verified by the test `name` in `tests/a.rs`.
fn tested(n: u32, name: &str) -> Link {
    Link {
        node: key(n),
        reviewed: None,
        evidence: Evidence::Tests(vec![Test {
            file: "tests/a.rs".to_owned(),
            name: name.to_owned(),
        }]),
        position: None,
    }
}

/// A link to node `n` reviewed against its current hash.
fn reviewed(document: &Document, n: u32, name: &str) -> Link {
    let mut link = tested(n, name);
    let id = hashes(document)
        .nodes
        .keys()
        .copied()
        .find(|id| id.key() == key(n))
        .unwrap();
    link.reviewed = Some(hash_of(document, id));
    link
}

fn run(document: &Document, links: Vec<Link>, files: &Files) -> Reconciled {
    reconcile(document, &hashes(document), &Trace { links }, files)
}

/// The rule and node of each finding, in order.
fn found(reconciled: &Reconciled) -> Vec<(TraceRule, Key)> {
    reconciled
        .findings
        .iter()
        .map(|finding| (finding.rule, finding.node))
        .collect()
}

/// The three nodes in scope, each linked to a test that `tests/a.rs` holds and reviewed.
fn all_reviewed(document: &Document) -> Vec<Link> {
    vec![
        reviewed(document, 5, "five"),
        reviewed(document, 9, "nine"),
        reviewed(document, 7, "seven"),
    ]
}

const SOURCE: &str = "fn five() {}\nfn nine() {}\nfn seven() {}\n";

#[test]
fn a_reviewed_link_to_a_found_test_for_every_node_in_scope_yields_no_finding() {
    let document = small_document();
    let reconciled = run(&document, all_reviewed(&document), &files(SOURCE));
    assert_eq!(found(&reconciled), []);
    assert_eq!((reconciled.in_scope, reconciled.linked), (3, 3));
}

#[test]
fn each_node_in_scope_without_a_link_is_tr_001_in_document_order() {
    let reconciled = run(&small_document(), Vec::new(), &Files::default());
    assert_eq!(
        found(&reconciled),
        [
            (TraceRule::Tr001, key(5)),
            (TraceRule::Tr001, key(9)),
            (TraceRule::Tr001, key(7))
        ]
    );
    assert_eq!((reconciled.in_scope, reconciled.linked), (3, 0));
    assert_eq!(TraceRule::Tr001.severity(), Severity::Info);
}

#[test]
fn a_constraint_or_quality_attribute_is_in_scope_only_when_proved() {
    let mut document = small_document();
    document.components[0].component.quality_attributes[0].verification_fate =
        Some(VerificationFate::Proved);
    document.products[0].feature_groups[0].constraints[0].verification_fate =
        Some(VerificationFate::Monitored);
    document.components[0].component.constraints[0].verification_fate = None;
    let nodes: Vec<Key> = run(&document, Vec::new(), &Files::default())
        .findings
        .iter()
        .map(|finding| finding.node)
        .collect();
    assert_eq!(nodes, [key(5), key(8)]);
}

#[test]
fn a_link_without_reviewed_is_tr_002_and_not_tr_003() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    links[0].reviewed = None;
    assert_eq!(
        found(&run(&document, links, &files(SOURCE))),
        [(TraceRule::Tr002, key(5))]
    );
    assert_eq!(TraceRule::Tr002.severity(), Severity::Warning);
}

#[test]
fn a_link_reviewed_before_its_node_changed_is_tr_003() {
    let mut document = small_document();
    let links = all_reviewed(&document);
    let criterion =
        &mut document.products[0].feature_groups[0].features[0].requirements[0].criteria[0];
    criterion.name = "Every node record carries its `key` field.".to_owned();
    assert_eq!(
        found(&run(&document, links, &files(SOURCE))),
        [(TraceRule::Tr003, key(5))]
    );
    assert_eq!(TraceRule::Tr003.severity(), Severity::Warning);
}

#[test]
fn a_link_whose_node_changed_only_its_label_is_not_tr_003() {
    let mut document = small_document();
    let links = all_reviewed(&document);
    document
        .bindings
        .bind(CriterionId::new(key(5)), label("AC-7"))
        .unwrap();
    assert_eq!(found(&run(&document, links, &files(SOURCE))), []);
}

#[test]
fn a_test_whose_file_does_not_exist_is_tr_004_naming_the_file_and_test() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    let Evidence::Tests(tests) = &mut links[1].evidence else {
        unreachable!()
    };
    tests[0].file = "tests/gone.rs".to_owned();
    let reconciled = run(&document, links, &files(SOURCE));
    assert_eq!(found(&reconciled), [(TraceRule::Tr004, key(9))]);
    let finding = &reconciled.findings[0];
    assert_eq!(finding.test.as_ref().unwrap().file, "tests/gone.rs");
    assert_eq!(finding.message, "the file `tests/gone.rs` does not exist");
    assert_eq!(TraceRule::Tr004.severity(), Severity::Warning);
}

#[test]
fn a_test_file_that_is_not_text_or_cannot_be_read_is_tr_004_saying_why() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    for (link, file) in [(1, "tests/binary.rs"), (2, "tests/locked.rs")] {
        let Evidence::Tests(tests) = &mut links[link].evidence else {
            unreachable!()
        };
        tests[0].file = file.to_owned();
    }
    let mut files = files(SOURCE);
    files
        .0
        .insert("tests/binary.rs".to_owned(), TestFile::NotText);
    let denied = TestFile::Unreadable("permission denied".to_owned());
    files.0.insert("tests/locked.rs".to_owned(), denied);
    let reconciled = run(&document, links, &files);
    assert_eq!(
        found(&reconciled),
        [(TraceRule::Tr004, key(9)), (TraceRule::Tr004, key(7))]
    );
    let messages: Vec<&str> = reconciled
        .findings
        .iter()
        .map(|finding| finding.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "the file `tests/binary.rs` is not UTF-8 text",
            "the file `tests/locked.rs` cannot be read: permission denied",
        ]
    );
}

#[test]
fn a_test_name_found_only_inside_a_longer_word_is_tr_004() {
    let document = small_document();
    let source = "fn five_more() {}\nfn renine() {}\nfn seven() {}\n";
    assert_eq!(
        found(&run(&document, all_reviewed(&document), &files(source))),
        [(TraceRule::Tr004, key(5)), (TraceRule::Tr004, key(9))]
    );
}

#[test]
fn a_test_name_with_spaces_is_found_where_no_word_character_adjoins_it() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    links[0] = reviewed(&document, 5, "prints the root");
    let source = format!("{SOURCE}it(\"prints the root\", () => {{}});\n");
    assert_eq!(found(&run(&document, links.clone(), &files(&source))), []);
    let joined = format!("{SOURCE}it(\"reprints the roots\");\n");
    assert_eq!(
        found(&run(&document, links, &files(&joined))),
        [(TraceRule::Tr004, key(5))]
    );
}

#[test]
fn a_test_file_outside_the_project_is_tr_004() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    let Evidence::Tests(tests) = &mut links[2].evidence else {
        unreachable!()
    };
    tests[0].file = "../elsewhere/tests/a.rs".to_owned();
    let mut files = files(SOURCE);
    let elsewhere = TestFile::Text(SOURCE.to_owned());
    files
        .0
        .insert("../elsewhere/tests/a.rs".to_owned(), elsewhere);
    assert_eq!(
        found(&run(&document, links, &files)),
        [(TraceRule::Tr004, key(7))]
    );
}

#[test]
fn a_link_to_an_unknown_key_or_a_node_out_of_scope_is_tr_005_alone() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    links.push(tested(900, "five"));
    links.push(tested(8, "five"));
    let reconciled = run(&document, links, &files(SOURCE));
    // QA-1 (8) is the last node of the tree; key 900 no node carries.
    assert_eq!(
        found(&reconciled),
        [(TraceRule::Tr005, key(8)), (TraceRule::Tr005, key(900))]
    );
    assert_eq!((reconciled.in_scope, reconciled.linked), (3, 3));
    assert_eq!(TraceRule::Tr005.severity(), Severity::Warning);
}

#[test]
fn findings_are_ordered_by_node_then_rule() {
    let document = small_document();
    let mut links = vec![tested(7, "missing"), tested(5, "missing")];
    links.push(tested(900, "x"));
    links.push(tested(800, "x"));
    assert_eq!(
        found(&run(&document, links, &files(SOURCE))),
        [
            (TraceRule::Tr002, key(5)),
            (TraceRule::Tr004, key(5)),
            (TraceRule::Tr001, key(9)),
            (TraceRule::Tr002, key(7)),
            (TraceRule::Tr004, key(7)),
            (TraceRule::Tr005, key(800)),
            (TraceRule::Tr005, key(900)),
        ]
    );
}

#[test]
fn two_missing_tests_of_one_link_are_reported_in_the_link_s_order() {
    let document = small_document();
    let mut link = tested(5, "second");
    let Evidence::Tests(tests) = &mut link.evidence else {
        unreachable!()
    };
    tests.insert(
        0,
        Test {
            file: "tests/a.rs".to_owned(),
            name: "first".to_owned(),
        },
    );
    let reconciled = run(&document, vec![link], &files(SOURCE));
    let names: Vec<&str> = reconciled
        .findings
        .iter()
        .filter_map(|finding| finding.test.as_ref())
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(names, ["first", "second"]);
}

#[test]
fn accepting_a_link_sets_reviewed_to_the_node_s_hash_and_changes_nothing_else() {
    let document = small_document();
    let mut trace = Trace {
        links: vec![tested(5, "five"), tested(9, "nine")],
    };
    let before = trace.clone();
    accept(
        &mut trace,
        &[key(5)],
        &document,
        &hashes(&document),
        &files(SOURCE),
    )
    .unwrap();
    assert_eq!(
        trace.links[0].reviewed,
        Some(hash_of(&document, criterion_id()))
    );
    assert_eq!(trace.links[0].evidence, before.links[0].evidence);
    assert_eq!(trace.links[1], before.links[1]);
}

#[test]
fn a_link_that_cannot_be_accepted_leaves_every_link_unchanged() {
    let document = small_document();
    let mut trace = Trace {
        links: vec![tested(5, "five"), tested(9, "absent"), tested(8, "five")],
    };
    let before = trace.clone();
    let refusals = accept(
        &mut trace,
        &[key(5), key(9), key(8), key(7), key(900)],
        &document,
        &hashes(&document),
        &files(SOURCE),
    )
    .unwrap_err();
    assert_eq!(
        refusals,
        [
            Refusal::Finding(key(9), TraceRule::Tr004),
            Refusal::Finding(key(8), TraceRule::Tr005),
            Refusal::NoLink(key(7)),
            Refusal::NoNode(key(900)),
        ]
    );
    assert_eq!(trace, before);
}

#[test]
fn accepting_the_stale_links_skips_a_link_whose_test_cannot_be_found() {
    let mut document = small_document();
    let mut links = all_reviewed(&document);
    links[1] = tested(9, "absent");
    links[2].reviewed = None;
    let mut trace = Trace { links };
    let criterion =
        &mut document.products[0].feature_groups[0].features[0].requirements[0].criteria[0];
    criterion.name = "Changed.".to_owned();
    let accepted = accept_stale(&mut trace, &document, &hashes(&document), &files(SOURCE));
    assert_eq!(accepted, [key(5), key(7)]);
    assert_eq!(trace.links[1].reviewed, None);
    assert_eq!(
        found(&run(&document, trace.links, &files(SOURCE))),
        [(TraceRule::Tr002, key(9)), (TraceRule::Tr004, key(9))]
    );
}

//! The reconciliation findings and accepting a link (the Feature "Reconciliation findings"),
//! over the small document: its nodes in scope are criterion 5, CON-2 (9) and CON-1 (7); QA-1
//! (8) and QA-2 (10) are sampled, and every other node is of another kind.

use crate::common::trace::{
    Files, SOURCE, all_reviewed, files, found, hashes, reviewed, run, test, tested, tests_of,
};
use crate::common::{key, label, small_document};
use loftspec::attributes::VerificationFate;
use loftspec::identity::Key;
use loftspec::ids::CriterionId;
use loftspec::lint::Severity;
use loftspec::trace::{Refusal, TestFile, Trace, TraceRule, accept, accept_stale};
use sha2::{Digest as _, Sha256};

/// The digest of a test whose canonical text is `canonical`.
fn digest_of(canonical: &str) -> loftspec::canonical::Digest {
    format!("{:x}", Sha256::digest(canonical)).parse().unwrap()
}

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
    let tests = tests_of(&mut links[1]);
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
        let tests = tests_of(&mut links[link]);
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
        [
            (TraceRule::Tr004, key(5)),
            (TraceRule::Tr008, key(5)),
            (TraceRule::Tr004, key(9))
        ]
    );
}

#[test]
fn a_test_name_with_spaces_is_found_where_no_word_character_adjoins_it() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    links[0] = reviewed(&document, 5, "prints the root");
    let source = format!("{SOURCE}it(\"prints the root\", () => {{}});\n");
    // The stand-in grammar locates no call, so the test is found only as a word.
    assert_eq!(
        found(&run(&document, links.clone(), &files(&source))),
        [(TraceRule::Tr007, key(5))]
    );
    let joined = format!("{SOURCE}it(\"reprints the roots\");\n");
    assert_eq!(
        found(&run(&document, links, &files(&joined))),
        [(TraceRule::Tr004, key(5)), (TraceRule::Tr008, key(5))]
    );
}

#[test]
fn a_test_file_outside_the_project_is_tr_004() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    let tests = tests_of(&mut links[2]);
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
            (TraceRule::Tr008, key(5)),
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
    let tests = tests_of(&mut link);
    tests.insert(0, test("first"));
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
fn accepting_a_link_sets_reviewed_and_its_tests_bodies_and_changes_nothing_else() {
    let document = small_document();
    let mut trace = Trace {
        links: vec![tested(5, "five")],
    };
    accept(
        &mut trace,
        &[key(5)],
        &document,
        &hashes(&document),
        &files(SOURCE),
    )
    .unwrap();
    let mut expected = reviewed(&document, 5, "five");
    tests_of(&mut expected)[0].body = Some(digest_of("fn five() { key }"));
    assert_eq!(trace.links, [expected]);
}

#[test]
fn accepting_a_link_changes_no_other_link_but_records_a_body_a_test_lacks() {
    let document = small_document();
    let mut recorded = tested(7, "seven");
    tests_of(&mut recorded)[0].body = Some(digest_of("an older seven"));
    let mut trace = Trace {
        links: vec![tested(5, "five"), tested(9, "nine"), recorded.clone()],
    };
    accept(
        &mut trace,
        &[key(5)],
        &document,
        &hashes(&document),
        &files(SOURCE),
    )
    .unwrap();
    let mut nine = tested(9, "nine");
    tests_of(&mut nine)[0].body = Some(digest_of("fn nine() {}"));
    assert_eq!(trace.links[1..], [nine, recorded]);
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

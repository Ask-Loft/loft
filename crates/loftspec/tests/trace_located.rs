//! Located tests (the Features "Locating a test" and "Reconciliation findings"): a test's digest
//! and locations, `TR-006` a test that changed after it was recorded, `TR-007` a test found only
//! as a word, `TR-008` a literal of the node's name that none of its tests holds, and the bodies
//! that accepting records. The stand-in grammar of `common::trace` parses files ending in `.rs`.

use std::collections::HashMap;

use crate::common::trace::{
    Files, SOURCE, all_reviewed, files, found, hashes, renamed, reviewed, run, test, tested,
    tests_of,
};
use crate::common::{key, small_document};
use loftspec::canonical::Digest;
use loftspec::lint::Severity;
use loftspec::trace::{Evidence, Found, Lines, TestFile, Trace, TraceRule, accept, accept_stale};
use sha2::{Digest as _, Sha256};

/// The SHA-256 of `canonical`.
fn digest_of(canonical: &str) -> Digest {
    format!("{:x}", Sha256::digest(canonical)).parse().unwrap()
}

/// The rules of the findings on criterion 5, in order.
fn on_five(reconciled: &loftspec::trace::Reconciled) -> Vec<TraceRule> {
    found(reconciled)
        .into_iter()
        .filter(|(_, node)| *node == key(5))
        .map(|(rule, _)| rule)
        .collect()
}

/// `files` with `path` holding `text` as well.
fn with(mut files: Files, path: &str, text: &str) -> Files {
    files
        .0
        .insert(path.to_owned(), TestFile::Text(text.to_owned()));
    files
}

#[test]
fn a_located_test_s_digest_is_the_sha_256_of_its_canonical_text() {
    let document = small_document();
    let reconciled = run(&document, all_reviewed(&document), &files(SOURCE));
    let expected = Found::Located {
        lines: vec![Lines::new(1, 1)],
        digest: digest_of("fn five() { key }"),
    };
    assert_eq!(reconciled.found[0], [expected]);
    let Found::Located { digest, .. } = &reconciled.found[0][0] else {
        unreachable!()
    };
    let hex = digest.to_string();
    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
}

#[test]
fn a_test_located_twice_has_both_locations_and_one_digest_over_both() {
    let document = small_document();
    let source = format!("{SOURCE}fn five() {{ again }}\n");
    let reconciled = run(&document, all_reviewed(&document), &files(&source));
    let expected = Found::Located {
        lines: vec![Lines::new(1, 1), Lines::new(4, 4)],
        digest: digest_of("fn five() { key }\nfn five() { again }"),
    };
    assert_eq!(reconciled.found[0], [expected]);
}

#[test]
fn whitespace_between_tokens_leaves_the_digest_and_raises_no_tr_006() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    tests_of(&mut links[0])[0].body = Some(digest_of("fn five() { key }"));
    let spaced = "fn five()   {  key   }\nfn nine() {}\nfn seven() {}\n";
    assert_eq!(found(&run(&document, links, &files(spaced))), []);
}

#[test]
fn a_located_test_whose_digest_differs_from_its_body_is_tr_006() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    tests_of(&mut links[1])[0].body = Some(digest_of("fn nine() { an older body }"));
    let reconciled = run(&document, links, &files(SOURCE));
    assert_eq!(found(&reconciled), [(TraceRule::Tr006, key(9))]);
    let finding = &reconciled.findings[0];
    assert_eq!(finding.test.as_ref().unwrap().name, "nine");
    assert_eq!(
        finding.message,
        "`nine` in `tests/a.rs` changed after it was recorded"
    );
    assert_eq!(TraceRule::Tr006.severity(), Severity::Warning);
}

#[test]
fn a_test_without_body_is_not_tr_006() {
    let document = small_document();
    let reconciled = run(&document, all_reviewed(&document), &files(SOURCE));
    assert_eq!(found(&reconciled), []);
}

#[test]
fn a_test_found_only_as_a_word_is_tr_007_and_never_tr_006() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    let tests = tests_of(&mut links[1]);
    tests[0].file = "tests/a.py".to_owned();
    tests[0].body = Some(digest_of("an older body"));
    links[2] = reviewed(&document, 7, "a call");
    let files = with(
        files(&format!("{SOURCE}it(\"a call\");\n")),
        "tests/a.py",
        "def nine(): pass\n",
    );
    let reconciled = run(&document, links, &files);
    assert_eq!(
        found(&reconciled),
        [(TraceRule::Tr007, key(9)), (TraceRule::Tr007, key(7))]
    );
    assert_eq!(
        reconciled.findings[0].message,
        "`nine` is found in `tests/a.py` only as a whole word, so its content is not tracked: \
         no grammar parses the file"
    );
    assert_eq!(
        reconciled.findings[1].message,
        "`a call` is found in `tests/a.rs` only as a whole word, so its content is not tracked: \
         no test in the file carries the name"
    );
    assert_eq!(TraceRule::Tr007.severity(), Severity::Info);
}

#[test]
fn a_node_whose_literals_no_test_holds_is_tr_008_naming_them_in_order() {
    let document = renamed("The `b` and `a` fields follow `key`, and an open ` stays prose.");
    let reconciled = run(
        &document,
        vec![reviewed(&document, 5, "five")],
        &files(SOURCE),
    );
    let anchors: Vec<_> = reconciled
        .findings
        .iter()
        .filter(|finding| finding.rule == TraceRule::Tr008)
        .collect();
    assert_eq!(anchors.len(), 1);
    assert_eq!(anchors[0].node, key(5));
    assert_eq!(
        anchors[0].message,
        "no test of the link holds `b`, `a` of the node's name"
    );
    assert_eq!(TraceRule::Tr008.severity(), Severity::Info);
}

#[test]
fn the_source_searched_is_the_located_test_or_the_whole_file_of_a_word() {
    let document = renamed("The `nine` and `tail` literals.");
    // `nine` is the text of another test's location, not of `five`'s.
    let located = run(
        &document,
        vec![reviewed(&document, 5, "five")],
        &files(SOURCE),
    );
    assert_eq!(on_five(&located), [TraceRule::Tr008]);
    // Found only as a word, the test's whole file is its source.
    let word = with(Files::default(), "tests/a.py", "five nine tail\n");
    let mut link = reviewed(&document, 5, "five");
    tests_of(&mut link)[0].file = "tests/a.py".to_owned();
    assert_eq!(
        on_five(&run(&document, vec![link], &word)),
        [TraceRule::Tr007]
    );
}

#[test]
fn a_test_with_tr_004_has_no_source_and_an_unverified_link_has_no_tr_008() {
    let document = small_document();
    let mut gone = reviewed(&document, 5, "five");
    tests_of(&mut gone)[0].file = "tests/gone.rs".to_owned();
    assert_eq!(
        on_five(&run(&document, vec![gone], &files(SOURCE))),
        [TraceRule::Tr004, TraceRule::Tr008]
    );
    let mut unverified = reviewed(&document, 5, "five");
    unverified.evidence = Evidence::Unverified("By hand.".to_owned());
    let reconciled = run(&document, vec![unverified], &files(SOURCE));
    assert_eq!(on_five(&reconciled), []);
}

#[test]
fn a_link_out_of_scope_yields_no_tr_006_tr_007_or_tr_008() {
    // QA-1 (8) is sampled, so out of scope; its link has a stale body, a test found only as a
    // word, and a literal its tests do not hold.
    let mut document = small_document();
    let quality = &mut document.components[0].component.quality_attributes[0];
    "The `absent` literal holds.".clone_into(&mut quality.name);
    let mut link = tested(8, "nine");
    tests_of(&mut link)[0].body = Some(digest_of("an older body"));
    let mut word = test("five");
    word.file = "tests/a.py".to_owned();
    tests_of(&mut link).push(word);
    let files = with(files(SOURCE), "tests/a.py", "five\n");
    let on_eight = |document: &loftspec::document::Document| -> Vec<TraceRule> {
        found(&run(document, vec![link.clone()], &files))
            .into_iter()
            .filter(|(_, node)| *node == key(8))
            .map(|(rule, _)| rule)
            .collect()
    };
    assert_eq!(on_eight(&document), [TraceRule::Tr005]);
    // In scope, the same link yields each of them.
    document.components[0].component.quality_attributes[0].verification_fate =
        Some(loftspec::attributes::VerificationFate::Proved);
    assert_eq!(
        on_eight(&document),
        [
            TraceRule::Tr002,
            TraceRule::Tr006,
            TraceRule::Tr007,
            TraceRule::Tr008
        ]
    );
}

#[test]
fn accepting_a_link_keeps_the_body_of_a_test_found_only_as_a_word() {
    let document = small_document();
    let mut link = tested(5, "five");
    let tests = tests_of(&mut link);
    tests[0].file = "tests/a.py".to_owned();
    tests[0].body = Some(digest_of("older"));
    let mut trace = Trace { links: vec![link] };
    let files = with(Files::default(), "tests/a.py", "five key\n");
    accept(&mut trace, &[key(5)], &document, &hashes(&document), &files).unwrap();
    assert_eq!(
        tests_of(&mut trace.links[0])[0].body,
        Some(digest_of("older"))
    );
}

#[test]
fn a_test_found_only_as_a_word_gets_no_body_when_links_are_accepted() {
    let document = small_document();
    let mut link = tested(5, "five");
    tests_of(&mut link)[0].file = "tests/a.py".to_owned();
    let mut trace = Trace {
        links: vec![link, tested(9, "nine")],
    };
    let files = with(files(SOURCE), "tests/a.py", "five key\n");
    accept(
        &mut trace,
        &[key(5), key(9)],
        &document,
        &hashes(&document),
        &files,
    )
    .unwrap();
    assert_eq!(tests_of(&mut trace.links[0])[0].body, None);
    assert!(tests_of(&mut trace.links[1])[0].body.is_some());
}

#[test]
fn accepting_the_stale_links_accepts_a_test_that_changed_and_records_its_body() {
    let document = small_document();
    let mut links = all_reviewed(&document);
    tests_of(&mut links[1])[0].body = Some(digest_of("older"));
    let mut trace = Trace { links };
    let accepted = accept_stale(&mut trace, &document, &hashes(&document), &files(SOURCE));
    assert_eq!(accepted, [key(9)]);
    assert_eq!(
        tests_of(&mut trace.links[1])[0].body,
        Some(digest_of("fn nine() {}"))
    );
    assert_eq!(found(&run(&document, trace.links, &files(SOURCE))), []);
}

#[test]
fn a_file_is_read_once_for_all_its_tests() {
    struct Counting(std::cell::RefCell<HashMap<String, usize>>);
    impl loftspec::trace::TestFiles for Counting {
        fn read(&self, path: &str) -> TestFile {
            *self.0.borrow_mut().entry(path.to_owned()).or_default() += 1;
            TestFile::Text(SOURCE.to_owned())
        }
    }
    let document = small_document();
    let counting = Counting(std::cell::RefCell::default());
    let trace = Trace {
        links: all_reviewed(&document),
    };
    loftspec::trace::reconcile(&document, &hashes(&document), &trace, &counting);
    assert_eq!(counting.0.borrow()["tests/a.rs"], 1);
}

#[test]
fn a_name_is_found_as_a_whole_word_where_it_overlaps_a_part_of_a_word() {
    let document = small_document();
    let mut link = reviewed(&document, 5, "go to go");
    tests_of(&mut link)[0].file = "tests/a.txt".to_owned();
    let files = with(Files::default(), "tests/a.txt", "ergo to go to go key\n");
    assert_eq!(
        on_five(&run(&document, vec![link], &files)),
        [TraceRule::Tr007]
    );
}

#[test]
fn a_path_that_steps_up_and_back_inside_the_project_is_inside_it() {
    let document = small_document();
    let mut inside = reviewed(&document, 5, "five");
    tests_of(&mut inside)[0].file = "tests/../tests/a.rs".to_owned();
    let up_and_back = with(files(SOURCE), "tests/../tests/a.rs", SOURCE);
    assert_eq!(on_five(&run(&document, vec![inside], &up_and_back)), []);
    let mut outside = reviewed(&document, 5, "five");
    tests_of(&mut outside)[0].file = "tests/../../a.rs".to_owned();
    let up_and_out = with(files(SOURCE), "tests/../../a.rs", SOURCE);
    assert_eq!(
        on_five(&run(&document, vec![outside], &up_and_out)),
        [TraceRule::Tr004, TraceRule::Tr008]
    );
}

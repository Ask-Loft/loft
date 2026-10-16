//! Tests whose content can no longer be checked (the Features "Reconciliation findings" and
//! "Locating a test"): `TR-009` a test whose body was recorded but that is found only as a whole
//! word, and `TR-010` a test that a part of its file that the grammar does not parse may cut
//! short. The stand-in grammar of `common::trace` reads each `// unparsed` as such a part.

use crate::common::trace::{
    Files, SOURCE, all_reviewed, files, found, hashes, run, tested, tests_of,
};
use crate::common::{key, small_document};
use loftspec::lint::Severity;
use loftspec::trace::{Refusal, Trace, TraceRule, accept, accept_stale};

/// The rules of the findings on criterion 5, in order.
fn on_five(reconciled: &loftspec::trace::Reconciled) -> Vec<TraceRule> {
    found(reconciled)
        .into_iter()
        .filter(|(_, node)| *node == key(5))
        .map(|(rule, _)| rule)
        .collect()
}

/// The three nodes in scope reviewed, with the body each test has in `files`, so that nothing is
/// found until the files change.
fn recorded(files: &Files) -> Trace {
    let document = small_document();
    let mut trace = Trace {
        links: all_reviewed(&document),
    };
    accept_stale(&mut trace, &document, &hashes(&document), files);
    assert_eq!(found(&run(&document, trace.links.clone(), files)), []);
    trace
}

#[test]
fn a_recorded_test_found_only_as_a_word_is_tr_009_saying_why_and_not_tr_007() {
    let document = small_document();
    let trace = recorded(&files(SOURCE));
    // `nine` is renamed, and its old name is left in a comment.
    let renamed = files("fn five() { key }\n// was nine\nfn ten() {}\nfn seven() {}\n");
    let reconciled = run(&document, trace.links, &renamed);
    assert_eq!(found(&reconciled), [(TraceRule::Tr009, key(9))]);
    assert_eq!(
        reconciled.findings[0].message,
        "`nine` in `tests/a.rs` was recorded but is found only as a whole word, so its content \
         cannot be checked: no test in the file carries the name"
    );
    assert_eq!(reconciled.findings[0].test.as_ref().unwrap().name, "nine");
    assert_eq!(TraceRule::Tr009.severity(), Severity::Warning);
    assert!(TraceRule::Tr009.asks_review());
}

#[test]
fn accepting_the_stale_links_accepts_a_tr_009_link_and_removes_its_body() {
    let document = small_document();
    let mut trace = recorded(&files(SOURCE));
    let renamed = files("fn five() { key }\n// was nine\nfn ten() {}\nfn seven() {}\n");
    let accepted = accept_stale(&mut trace, &document, &hashes(&document), &renamed);
    assert_eq!(accepted, [key(9)]);
    assert_eq!(tests_of(&mut trace.links[1])[0].body, None);
    assert_eq!(
        found(&run(&document, trace.links, &renamed)),
        [(TraceRule::Tr007, key(9))]
    );
}

#[test]
fn a_test_that_holds_a_part_that_does_not_parse_is_tr_010_giving_where_and_what() {
    let document = small_document();
    let trace = recorded(&files(SOURCE));
    let broken = files("fn five() { key // unparsed )\nfn nine() {}\nfn seven() {}\n");
    let reconciled = run(&document, trace.links, &broken);
    assert_eq!(found(&reconciled), [(TraceRule::Tr010, key(5))]);
    assert_eq!(
        reconciled.findings[0].message,
        "`five` in `tests/a.rs` cannot be located: the file does not parse at 1:17, at \
         `// unparsed )`"
    );
    assert_eq!(TraceRule::Tr010.severity(), Severity::Warning);
    assert!(TraceRule::Tr010.refuses_accept());
}

#[test]
fn a_test_followed_by_a_part_that_does_not_parse_before_the_next_test_is_tr_010() {
    let document = small_document();
    let trace = recorded(&files(SOURCE));
    let broken = files("fn five() { key }\n// unparsed\nfn nine() {}\nfn seven() {}\n");
    assert_eq!(
        found(&run(&document, trace.links, &broken)),
        [(TraceRule::Tr010, key(5))]
    );
}

#[test]
fn a_part_that_does_not_parse_after_the_last_test_cuts_that_test_short() {
    let document = small_document();
    let trace = recorded(&files(SOURCE));
    let broken = files(&format!("{SOURCE}// unparsed\n"));
    assert_eq!(
        found(&run(&document, trace.links, &broken)),
        [(TraceRule::Tr010, key(7))]
    );
}

#[test]
fn a_test_neither_holding_nor_followed_by_a_part_that_does_not_parse_is_located() {
    let document = small_document();
    let trace = recorded(&files(SOURCE));
    let broken = files(&format!("// unparsed\n{SOURCE}"));
    assert_eq!(found(&run(&document, trace.links, &broken)), []);
}

#[test]
fn a_tr_010_test_yields_no_tr_006_tr_007_or_tr_009() {
    let document = small_document();
    let trace = recorded(&files(SOURCE));
    // `five` changed and is cut short: only TR-010 is said of it.
    let broken = files("fn five() { other key // unparsed\nfn nine() {}\nfn seven() {}\n");
    assert_eq!(
        on_five(&run(&document, trace.links, &broken)),
        [TraceRule::Tr010]
    );
}

#[test]
fn a_tr_010_link_cannot_be_accepted_and_keeps_its_reviewed_and_bodies() {
    let document = small_document();
    let mut trace = recorded(&files(SOURCE));
    let before = trace.clone();
    let broken = files("fn five() { key // unparsed\nfn nine() {}\nfn seven() {}\n");
    let refused = accept(
        &mut trace,
        &[key(5)],
        &document,
        &hashes(&document),
        &broken,
    );
    assert_eq!(
        refused,
        Err(vec![Refusal::Finding(key(5), TraceRule::Tr010)])
    );
    assert_eq!(trace, before);
    assert_eq!(
        accept_stale(&mut trace, &document, &hashes(&document), &broken),
        []
    );
    assert_eq!(trace, before);
}

#[test]
fn a_tr_007_in_a_file_that_does_not_parse_says_where() {
    let document = small_document();
    let links = vec![tested(5, "a call")];
    let broken = files("fn five() { key }\n// unparsed\nit(\"a call\");\n");
    let reconciled = run(&document, links, &broken);
    let messages: Vec<&str> = reconciled
        .findings
        .iter()
        .filter(|finding| finding.rule == TraceRule::Tr007)
        .map(|finding| finding.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "`a call` is found in `tests/a.rs` only as a whole word, so its content is not \
          tracked: no test in the file carries the name, and the file does not parse at 2:1, at \
          `// unparsed`"
        ]
    );
}

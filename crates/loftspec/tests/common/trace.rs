//! The reconciler's test files held in memory, and links over the small document: its nodes in
//! scope are criterion 5, CON-2 (9) and CON-1 (7).

use std::collections::HashMap;

use super::{key, small_document};
use loftspec::canonical::Digest;
use loftspec::document::Document;
use loftspec::hash::{DocumentHashes, hash_document};
use loftspec::identity::Key;
use loftspec::trace::{
    Evidence, Lines, Link, LocatedTest, Reconciled, Test, TestFile, TestFiles, Trace, TraceRule,
    reconcile,
};

/// Test files held in memory; a path not held is missing. A stand-in grammar parses each file
/// ending in `.rs`: every line that starts with `fn <name>(` is a test named `<name>`, its
/// canonical text the line's words joined by one space.
#[derive(Default)]
pub struct Files(pub HashMap<String, TestFile>);

impl TestFiles for Files {
    fn read(&self, path: &str) -> TestFile {
        self.0.get(path).cloned().unwrap_or(TestFile::Missing)
    }

    fn tests(&self, path: &str, text: &str) -> Option<Vec<LocatedTest>> {
        let rust = std::path::Path::new(path)
            .extension()
            .is_some_and(|extension| extension == "rs");
        rust.then(|| {
            text.lines()
                .enumerate()
                .filter_map(|(at, line)| {
                    let name = line.strip_prefix("fn ")?.split('(').next()?;
                    Some(LocatedTest {
                        name: name.to_owned(),
                        lines: Lines {
                            first: at + 1,
                            last: at + 1,
                        },
                        canonical: line.split_whitespace().collect::<Vec<_>>().join(" "),
                        source: line.to_owned(),
                    })
                })
                .collect()
        })
    }
}

/// One file, `tests/a.rs`, holding `text`.
pub fn files(text: &str) -> Files {
    Files(HashMap::from([(
        "tests/a.rs".to_owned(),
        TestFile::Text(text.to_owned()),
    )]))
}

/// Three tests the stand-in grammar locates, `five` holding the literal of criterion 5's name.
pub const SOURCE: &str = "fn five() { key }\nfn nine() {}\nfn seven() {}\n";

pub fn hashes(document: &Document) -> DocumentHashes {
    hash_document(document).unwrap()
}

/// A test named `name` in `tests/a.rs`, with no `body`.
pub fn test(name: &str) -> Test {
    Test {
        file: "tests/a.rs".to_owned(),
        name: name.to_owned(),
        body: None,
    }
}

/// A link to node `n`, verified by the test `name` in `tests/a.rs`.
pub fn tested(n: u32, name: &str) -> Link {
    Link {
        node: key(n),
        reviewed: None,
        evidence: Evidence::Tests(vec![test(name)]),
        position: None,
    }
}

/// The content hash of the node keyed `key(n)`.
pub fn hash_of(document: &Document, n: u32) -> Digest {
    let hashes = hashes(document);
    let id = hashes.nodes.keys().find(|id| id.key() == key(n)).unwrap();
    hashes.nodes[id]
}

/// A link to node `n` reviewed against its current hash.
pub fn reviewed(document: &Document, n: u32, name: &str) -> Link {
    Link {
        reviewed: Some(hash_of(document, n)),
        ..tested(n, name)
    }
}

pub fn run(document: &Document, links: Vec<Link>, files: &Files) -> Reconciled {
    reconcile(document, &hashes(document), &Trace { links }, files)
}

/// The rule and node of each finding, in order.
pub fn found(reconciled: &Reconciled) -> Vec<(TraceRule, Key)> {
    reconciled
        .findings
        .iter()
        .map(|finding| (finding.rule, finding.node))
        .collect()
}

/// The three nodes in scope, each linked to a test that `tests/a.rs` holds and reviewed.
pub fn all_reviewed(document: &Document) -> Vec<Link> {
    vec![
        reviewed(document, 5, "five"),
        reviewed(document, 9, "nine"),
        reviewed(document, 7, "seven"),
    ]
}

/// The small document with criterion 5 renamed to `name`.
pub fn renamed(name: &str) -> Document {
    let mut document = small_document();
    let criterion =
        &mut document.products[0].feature_groups[0].features[0].requirements[0].criteria[0];
    name.clone_into(&mut criterion.name);
    document
}

/// The tests of `link`.
pub fn tests_of(link: &mut Link) -> &mut Vec<Test> {
    let Evidence::Tests(tests) = &mut link.evidence else {
        unreachable!()
    };
    tests
}

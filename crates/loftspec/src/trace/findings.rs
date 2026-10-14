//! The reconciliation findings: `TR-001` a node in scope that no link names, `TR-002` a link not
//! yet reviewed, `TR-003` a link reviewed before its node changed, `TR-004` a test that cannot be
//! found, `TR-005` a link whose node is not in scope. A test is found as a whole word in its
//! file; the reconciler runs no test and reads no runner's report.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Component, Path};

use super::{Evidence, Test, Trace, order, places};
use crate::document::Document;
use crate::hash::DocumentHashes;
use crate::identity::Key;
use crate::lint::Severity;
use crate::wire::wire_names;
use crate::yaml::Position;

/// A reconciliation rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TraceRule {
    Tr001,
    Tr002,
    Tr003,
    Tr004,
    Tr005,
}

wire_names!(TraceRule {
    Tr001 => "TR-001",
    Tr002 => "TR-002",
    Tr003 => "TR-003",
    Tr004 => "TR-004",
    Tr005 => "TR-005",
});

impl TraceRule {
    /// `TR-001` is info; every rule that says a link is not true is a warning.
    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            Self::Tr001 => Severity::Info,
            Self::Tr002 | Self::Tr003 | Self::Tr004 | Self::Tr005 => Severity::Warning,
        }
    }

    /// Whether the rule asks for a review: `TR-002` and `TR-003`.
    #[must_use]
    pub const fn asks_review(self) -> bool {
        matches!(self, Self::Tr002 | Self::Tr003)
    }

    /// Whether the rule says the link is not true, so it cannot be accepted: `TR-004` and
    /// `TR-005`.
    #[must_use]
    pub const fn refuses_accept(self) -> bool {
        matches!(self, Self::Tr004 | Self::Tr005)
    }
}

impl fmt::Display for TraceRule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One finding, on the node a link or the tree names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceFinding {
    pub rule: TraceRule,
    pub node: Key,
    /// The test a `TR-004` finding is about.
    pub test: Option<Test>,
    /// Where the node's link is written in the trace file, for a finding on a link read from it.
    pub link: Option<Position>,
    pub message: String,
}

/// The files that hold the tests, by their paths relative to the directory that holds `spec/`.
pub trait TestFiles {
    /// The file's text; none when there is no such file or it cannot be read as text.
    fn text(&self, path: &str) -> Option<String>;
}

/// What the reconciler found, and how many nodes in scope a link names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciled {
    pub findings: Vec<TraceFinding>,
    pub in_scope: usize,
    pub linked: usize,
}

impl TraceFinding {
    fn new(rule: TraceRule, node: Key, link: Option<Position>, message: impl Into<String>) -> Self {
        Self {
            rule,
            node,
            test: None,
            link,
            message: message.into(),
        }
    }

    /// The rule's severity.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.rule.severity()
    }
}

/// Compares `trace` with `document`, whose hashes are `hashes`, and with the test files.
pub fn reconcile(
    document: &Document,
    hashes: &DocumentHashes,
    trace: &Trace,
    files: &impl TestFiles,
) -> Reconciled {
    let places = places(document);
    let mut read = HashMap::new();
    let mut findings = Vec::new();
    let mut linked = HashSet::new();
    for link in &trace.links {
        let finding =
            |rule, message: &str| TraceFinding::new(rule, link.node, link.position, message);
        let Some(place) = places.get(&link.node).filter(|place| place.in_scope) else {
            findings.push(finding(
                TraceRule::Tr005,
                if places.contains_key(&link.node) {
                    "the node is not in scope: only acceptance criteria and proved Constraints \
                     and Quality Attributes are"
                } else {
                    "no node of the tree carries this key"
                },
            ));
            continue;
        };
        linked.insert(link.node);
        match link.reviewed {
            None => findings.push(finding(TraceRule::Tr002, "the link has not been reviewed")),
            Some(reviewed) if reviewed != hashes.nodes[&place.id] => findings.push(finding(
                TraceRule::Tr003,
                "the node changed after the link was reviewed",
            )),
            Some(_) => {}
        }
        let tests = match &link.evidence {
            Evidence::Tests(tests) => tests.as_slice(),
            Evidence::Unverified(_) => &[],
        };
        for test in tests {
            if let Some(wrong) = missing(test, &mut read, files) {
                findings.push(TraceFinding {
                    test: Some(test.clone()),
                    ..finding(TraceRule::Tr004, &wrong)
                });
            }
        }
    }
    let mut in_scope = 0;
    for (key, _) in places.iter().filter(|(_, place)| place.in_scope) {
        in_scope += 1;
        if !linked.contains(key) {
            let message = "no link names this node";
            findings.push(TraceFinding::new(TraceRule::Tr001, *key, None, message));
        }
    }
    findings.sort_by_key(|finding| (order(&places, finding.node), finding.rule));
    Reconciled {
        findings,
        in_scope,
        linked: linked.len(),
    }
}

/// What is wrong with `test` when it cannot be found; each file is read once, into `read`.
fn missing<'a>(
    test: &'a Test,
    read: &mut HashMap<&'a str, Option<String>>,
    files: &impl TestFiles,
) -> Option<String> {
    let file = test.file.as_str();
    if !inside(file) {
        return Some(format!(
            "the file `{file}` lies outside the directory that holds `spec/`"
        ));
    }
    match read.entry(file).or_insert_with(|| files.text(file)) {
        None => Some(format!("the file `{file}` does not exist")),
        Some(held) if !occurs_whole(held, &test.name) => Some(format!(
            "`{}` does not occur in `{file}` as a whole word",
            test.name
        )),
        Some(_) => None,
    }
}

/// Whether `path` names a file under the directory that holds `spec/`: relative, and never
/// stepping up out of it.
fn inside(path: &str) -> bool {
    Path::new(path)
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

/// Whether `name` occurs in `text` where no letter, digit or underscore adjoins either end.
fn occurs_whole(text: &str, name: &str) -> bool {
    let word = |character: char| character.is_alphanumeric() || character == '_';
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

//! The reconciliation findings: `TR-001` a node in scope that no link names, `TR-002` a link not
//! yet reviewed, `TR-003` a link reviewed before its node changed, `TR-004` a test that cannot be
//! found, `TR-005` a link whose node is not in scope, `TR-006` a test whose content changed after
//! it was recorded, `TR-007` a test found only as a whole word, `TR-008` a node whose name holds a
//! literal that none of its tests holds, `TR-009` a test whose content was recorded but that is
//! found only as a whole word, `TR-010` a test that a part of its file that the grammar does not
//! parse may cut short. The reconciler runs no test and reads no runner's report.

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::found::{Found, Seeker, TestFiles, Untracked, literals};
use super::{Evidence, Link, Place, Test, Trace, names, order, places};
use crate::canonical::Digest;
use crate::document::Document;
use crate::hash::DocumentHashes;
use crate::identity::Key;
use crate::lint::Severity;
use crate::wire::wire_names;
use crate::yaml::Position;

/// A reconciliation rule; a later version may add rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TraceRule {
    Tr001,
    Tr002,
    Tr003,
    Tr004,
    Tr005,
    Tr006,
    Tr007,
    Tr008,
    Tr009,
    Tr010,
}

wire_names!(TraceRule {
    Tr001 => "TR-001",
    Tr002 => "TR-002",
    Tr003 => "TR-003",
    Tr004 => "TR-004",
    Tr005 => "TR-005",
    Tr006 => "TR-006",
    Tr007 => "TR-007",
    Tr008 => "TR-008",
    Tr009 => "TR-009",
    Tr010 => "TR-010",
});

impl TraceRule {
    /// `TR-001`, `TR-007` and `TR-008` are info; every rule that says a link is not true, or asks
    /// for a review, is a warning.
    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            Self::Tr001 | Self::Tr007 | Self::Tr008 => Severity::Info,
            Self::Tr002
            | Self::Tr003
            | Self::Tr004
            | Self::Tr005
            | Self::Tr006
            | Self::Tr009
            | Self::Tr010 => Severity::Warning,
        }
    }

    /// Whether the rule asks for a review: `TR-002`, `TR-003`, `TR-006` and `TR-009`.
    #[must_use]
    pub const fn asks_review(self) -> bool {
        matches!(self, Self::Tr002 | Self::Tr003 | Self::Tr006 | Self::Tr009)
    }

    /// Whether the rule says the link cannot be accepted: `TR-004` and `TR-005`, which say it is
    /// not true, and `TR-010`, which says its test cannot be read.
    #[must_use]
    pub const fn refuses_accept(self) -> bool {
        matches!(self, Self::Tr004 | Self::Tr005 | Self::Tr010)
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
    /// The test a `TR-004`, `TR-006`, `TR-007`, `TR-009` or `TR-010` finding is about.
    pub test: Option<Test>,
    /// Where the node's link is written in the trace file, for a finding on a link read from it.
    pub link: Option<Position>,
    pub message: String,
}

/// What the reconciler found, and how many nodes in scope a link names.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Reconciled {
    pub findings: Vec<TraceFinding>,
    pub in_scope: usize,
    pub linked: usize,
    /// What was found of each test, for each link in the order of the trace file; a link with
    /// `unverified` has none.
    pub found: Vec<Vec<Found>>,
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
    let names = names(document);
    let mut seeker = Seeker::new(files);
    let mut findings = Vec::new();
    let mut linked = HashSet::new();
    let mut found = Vec::new();
    for link in &trace.links {
        let tests = tests(link);
        let seen: Vec<Found> = tests.iter().map(|test| seeker.seek(test)).collect();
        if let Some(place) = places.get(&link.node).filter(|place| place.in_scope) {
            linked.insert(link.node);
            let literals = names
                .get(&link.node)
                .map_or_else(Vec::new, |name| literals(name));
            let checked = Checked {
                link,
                tests,
                found: &seen,
            };
            checked.findings(&mut findings, hashes.nodes[&place.id]);
            checked.anchors(&mut findings, &literals, &seeker);
        } else {
            findings.push(out_of_scope(link, &places));
        }
        found.push(seen);
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
        found,
    }
}

/// The tests a link names; none for a link with `unverified`.
fn tests(link: &Link) -> &[Test] {
    match &link.evidence {
        Evidence::Tests(tests) => tests,
        Evidence::Unverified(_) => &[],
    }
}

/// The `TR-005` finding of a link whose node is not in scope.
fn out_of_scope(link: &Link, places: &HashMap<Key, Place>) -> TraceFinding {
    let message = if places.contains_key(&link.node) {
        "the node is not in scope: only acceptance criteria and proved Constraints and Quality \
         Attributes are"
    } else {
        "no node of the tree carries this key"
    };
    TraceFinding::new(TraceRule::Tr005, link.node, link.position, message)
}

/// A link whose node is in scope, its tests, and what was found of each.
struct Checked<'a> {
    link: &'a Link,
    tests: &'a [Test],
    found: &'a [Found],
}

impl Checked<'_> {
    fn finding(&self, rule: TraceRule, test: Option<&Test>, message: String) -> TraceFinding {
        TraceFinding {
            test: test.cloned(),
            ..TraceFinding::new(rule, self.link.node, self.link.position, message)
        }
    }

    /// `TR-002` and `TR-003` on the link, then `TR-004`, `TR-006`, `TR-007`, `TR-009` and
    /// `TR-010` on its tests.
    fn findings(&self, findings: &mut Vec<TraceFinding>, hash: Digest) {
        match self.link.reviewed {
            None => findings.push(self.finding(
                TraceRule::Tr002,
                None,
                "the link has not been reviewed".to_owned(),
            )),
            Some(reviewed) if reviewed != hash => findings.push(self.finding(
                TraceRule::Tr003,
                None,
                "the node changed after the link was reviewed".to_owned(),
            )),
            Some(_) => {}
        }
        for (test, found) in self.tests.iter().zip(self.found) {
            let (rule, message) = match found {
                Found::Missing(wrong) => (TraceRule::Tr004, wrong.clone()),
                Found::Word(why) if test.body.is_some() => (
                    TraceRule::Tr009,
                    format!(
                        "`{}` in `{}` was recorded but is found only as a whole word, so its \
                         content cannot be checked: {}",
                        test.name,
                        test.file,
                        reason(why)
                    ),
                ),
                Found::Word(why) => (TraceRule::Tr007, untracked(test, why)),
                Found::Unparsed(error) => (
                    TraceRule::Tr010,
                    format!(
                        "`{}` in `{}` cannot be located: the file does not parse at {error}",
                        test.name, test.file
                    ),
                ),
                Found::Located { digest, .. } if test.body.is_some_and(|body| body != *digest) => (
                    TraceRule::Tr006,
                    format!(
                        "`{}` in `{}` changed after it was recorded",
                        test.name, test.file
                    ),
                ),
                Found::Located { .. } => continue,
            };
            findings.push(self.finding(rule, Some(test), message));
        }
    }

    /// `TR-008`: the literals of the node's name that the source of none of the link's tests
    /// holds.
    fn anchors<F: TestFiles>(
        &self,
        findings: &mut Vec<TraceFinding>,
        literals: &[&str],
        seeker: &Seeker<'_, F>,
    ) {
        if self.tests.is_empty() {
            return;
        }
        let absent: Vec<String> = literals
            .iter()
            .filter(|literal| {
                !self
                    .tests
                    .iter()
                    .zip(self.found)
                    .any(|(test, found)| seeker.holds(test, found, literal))
            })
            .map(|literal| format!("`{literal}`"))
            .collect();
        if !absent.is_empty() {
            let message = format!(
                "no test of the link holds {} of the node's name",
                absent.join(", ")
            );
            findings.push(self.finding(TraceRule::Tr008, None, message));
        }
    }
}

/// The message of the `TR-007` finding on `test`, found only as a word.
fn untracked(test: &Test, why: &Untracked) -> String {
    format!(
        "`{}` is found in `{}` only as a whole word, so its content is not tracked: {}",
        test.name,
        test.file,
        reason(why)
    )
}

/// Why a test found only as a word was not located.
fn reason(why: &Untracked) -> String {
    match why {
        Untracked::NoGrammar => "no grammar parses the file".to_owned(),
        Untracked::NoTest(None) => "no test in the file carries the name".to_owned(),
        Untracked::NoTest(Some(error)) => {
            format!("no test in the file carries the name, and the file does not parse at {error}")
        }
    }
}

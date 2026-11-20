//! Implementation reconciliation (the Feature group of that name): the trace file,
//! `loft.trace.yaml` beside `spec/`, links each node in scope to the tests that verify it, pinned
//! to the content hash its node had when the link was last reviewed, and records the digest each
//! test had when it was last recorded; the reconciler reports which links no longer hold and
//! accepts the links a reviewer has checked. The trace file enters no root. The reconciler reads
//! and parses no file itself: its caller gives it the trace file's bytes and a [`TestFiles`] port
//! to the files that hold the tests and to the tests each file holds.

mod accept;
mod findings;
mod found;
mod read;
mod write;

pub use accept::{Refusal, accept, accept_stale};
pub use findings::{Reconciled, TraceFinding, TraceRule, reconcile};
pub use found::{
    Found, Lines, LocatedTest, ParseError, ParsedFile, TestFile, TestFiles, Untracked,
};
pub use read::{FILE_FIELDS, LINK_FIELDS, TEST_FIELDS, read};
pub use write::write;

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::attributes::VerificationFate;
use crate::canonical::Digest;
use crate::codec::Problem;
use crate::copy::Copy;
use crate::document::{Document, Edge};
use crate::hash::DocumentHashes;
use crate::identity::Key;
use crate::ids::NodeId;
use crate::node::Node;
use crate::yaml::{Location, Position};

/// The trace file's name, in the directory that holds `spec/`.
pub const TRACE: &str = "loft.trace.yaml";

/// The trace file's `format`, versioned with its fields; the writer writes this one.
pub const FORMAT: &str = "loft-trace/2";

/// The `format` of a trace file written before tests had a `body`; it is still read.
pub const FORMAT_1: &str = "loft-trace/1";

/// The links of a trace file, in the order read or to be written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    pub links: Vec<Link>,
}

/// The tests that verify one node, or the reason none does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub node: Key,
    /// The node's content hash when the link was last reviewed; none before its first review.
    pub reviewed: Option<Digest>,
    pub evidence: Evidence,
    /// Where the link is written in the trace file; none for a link not read from one.
    pub position: Option<Position>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// At least one test.
    Tests(Vec<Test>),
    /// Why no test verifies the node.
    Unverified(String),
}

/// A test, by the file that holds it, relative to the directory that holds `spec/`, and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Test {
    pub file: String,
    pub name: String,
    /// The test's digest when it was last recorded; none before it was first located.
    pub body: Option<Digest>,
}

impl Test {
    /// The test `name` in `file`, with no digest recorded.
    #[must_use]
    pub fn new(file: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            body: None,
        }
    }
}

/// An error in the trace file, at its line and column where it has one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct TraceError {
    pub location: Option<Location>,
    pub problem: Problem,
}

impl fmt::Display for TraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location {
            Some(location) => write!(formatter, "{TRACE}:{location}: {}", self.problem),
            None => write!(formatter, "{TRACE}: {}", self.problem),
        }
    }
}

/// A keyed node's place in the tree: its index in document order, its ID, and whether the
/// reconciler expects a link for it.
#[derive(Debug, Clone, Copy)]
struct Place {
    index: usize,
    id: NodeId,
    in_scope: bool,
}

/// Every keyed node's place, by key: the document's own nodes in document order, then the nodes
/// of its copies, by handle in byte order and then in the order of the copies' files.
fn places(document: &Document) -> HashMap<Key, Place> {
    let mut places = HashMap::new();
    each_node(document, |node, in_scope| {
        if let Some(id) = node.id() {
            let index = places.len();
            places.insert(
                id.key(),
                Place {
                    index,
                    id,
                    in_scope,
                },
            );
        }
    });
    places
}

/// The name of each node in scope, by key.
fn names(document: &Document) -> HashMap<Key, String> {
    let mut names = HashMap::new();
    each_node(document, |node, in_scope| {
        if let Some(id) = node.id()
            && in_scope
        {
            names.insert(id.key(), node.name().into_owned());
        }
    });
    names
}

/// Visits every node the reconciler places, with whether it expects a link for it: the
/// document's own nodes, then each node of a copy, whose criteria it expects a link for where a
/// Component of the document provides the copied Interface (a foreign Interface it answers).
fn each_node<'a>(document: &'a Document, mut visit: impl FnMut(Node<'a>, bool)) {
    document.walk(&mut |node| visit(node, in_scope(node)));
    let provided: HashSet<Key> = (document.edges.iter())
        .filter_map(|edge| match edge {
            Edge::Provides { interface, .. } => Some(interface.key()),
            _ => None,
        })
        .collect();
    let mut copies: Vec<&Copy> = document.copies.iter().collect();
    copies.sort_by_key(|copy| (copy.handle.as_str(), copy.path()));
    for copy in copies {
        let answered = (copy.interface.id).is_some_and(|id| provided.contains(&id.key()));
        Node::Interface(&copy.interface).walk(&mut |node| {
            visit(node, answered && matches!(node, Node::Criterion(_)));
        });
    }
}

/// The content hash of a node the reconciler places: a node of the document, or of a copy.
fn hash_of(hashes: &DocumentHashes, id: NodeId) -> Digest {
    *hashes
        .nodes
        .get(&id)
        .or_else(|| hashes.foreign.get(&id))
        .expect("every placed node is hashed")
}

/// Whether the reconciler expects a link for `node`: every acceptance criterion, and every
/// Constraint and Quality Attribute whose verification fate is `proved`.
fn in_scope(node: Node<'_>) -> bool {
    let proved = |fate: Option<VerificationFate>| fate == Some(VerificationFate::Proved);
    match node {
        Node::Criterion(_) => true,
        Node::Constraint(constraint) => proved(constraint.verification_fate),
        Node::QualityAttribute(quality) => proved(quality.verification_fate),
        _ => false,
    }
}

/// The order of the links and findings of `key`: the nodes of the tree in document order, then
/// the keys no node carries, in the order of the keys.
fn order(places: &HashMap<Key, Place>, key: Key) -> (usize, Key) {
    (
        places.get(&key).map_or(usize::MAX, |place| place.index),
        key,
    )
}

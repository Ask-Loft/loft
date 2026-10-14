//! Implementation reconciliation (the Feature group of that name): the trace file,
//! `loft.trace.yaml` beside `spec/`, links each node in scope to the tests that verify it, pinned
//! to the content hash its node had when the link was last reviewed; the reconciler reports which
//! links no longer hold and accepts the links a reviewer has checked. The trace file enters no
//! root. The reconciler reads no file itself: its caller gives it the trace file's bytes and a
//! [`TestFiles`] port to the files that hold the tests.

mod accept;
mod findings;
mod read;
mod write;

pub use accept::{Refusal, accept, accept_stale};
pub use findings::{Reconciled, TestFile, TestFiles, TraceFinding, TraceRule, reconcile};
pub use read::{FILE_FIELDS, LINK_FIELDS, TEST_FIELDS, read};
pub use write::write;

use std::collections::HashMap;
use std::fmt;

use crate::attributes::VerificationFate;
use crate::canonical::Digest;
use crate::codec::Problem;
use crate::document::Document;
use crate::identity::Key;
use crate::ids::NodeId;
use crate::node::Node;
use crate::yaml::{Location, Position};

/// The trace file's name, in the directory that holds `spec/`.
pub const TRACE: &str = "loft.trace.yaml";

/// The trace file's `format`, versioned with its fields.
pub const FORMAT: &str = "loft-trace/1";

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
pub struct Test {
    pub file: String,
    pub name: String,
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

/// Every keyed node's place, by key.
fn places(document: &Document) -> HashMap<Key, Place> {
    let mut places = HashMap::new();
    document.walk(&mut |node| {
        if let Some(id) = node.id() {
            let index = places.len();
            let in_scope = in_scope(node);
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

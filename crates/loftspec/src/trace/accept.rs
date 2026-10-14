//! Accepting a link: its `reviewed` becomes its node's current content hash, which states that a
//! reviewer has checked its tests against the node as it now reads. A link with a `TR-004` or
//! `TR-005` finding is never accepted.

use std::collections::HashSet;
use std::fmt;

use super::{Trace, TraceRule, places, reconcile};
use crate::document::Document;
use crate::hash::DocumentHashes;
use crate::identity::Key;

use super::findings::TestFiles;

/// Why a node's link cannot be accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No node of the tree carries the key.
    NoNode(Key),
    /// The node has no link.
    NoLink(Key),
    /// The node's link has a finding of this rule: `TR-004` or `TR-005`.
    Finding(Key, TraceRule),
}

impl Refusal {
    /// The node the refusal is about.
    #[must_use]
    pub const fn key(&self) -> Key {
        match self {
            Self::NoNode(key) | Self::NoLink(key) | Self::Finding(key, _) => *key,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoNode(_) => formatter.write_str("no node of the tree carries this key"),
            Self::NoLink(_) => formatter.write_str("no link names this node"),
            Self::Finding(_, rule) => write!(formatter, "the link has a {rule} finding"),
        }
    }
}

/// Accepts the links of the nodes `keys`, all of them or none.
///
/// # Errors
///
/// Every node whose link cannot be accepted, and why; `trace` is then unchanged.
pub fn accept(
    trace: &mut Trace,
    keys: &[Key],
    document: &Document,
    hashes: &DocumentHashes,
    files: &impl TestFiles,
) -> Result<(), Vec<Refusal>> {
    let places = places(document);
    let untrue = untrue(trace, document, hashes, files);
    let refusals: Vec<Refusal> = keys
        .iter()
        .filter_map(|key| {
            if !places.contains_key(key) {
                Some(Refusal::NoNode(*key))
            } else if !trace.links.iter().any(|link| link.node == *key) {
                Some(Refusal::NoLink(*key))
            } else {
                let rule = untrue.iter().find(|(node, _)| node == key);
                rule.map(|(_, rule)| Refusal::Finding(*key, *rule))
            }
        })
        .collect();
    if !refusals.is_empty() {
        return Err(refusals);
    }
    let keys: HashSet<&Key> = keys.iter().collect();
    for link in trace
        .links
        .iter_mut()
        .filter(|link| keys.contains(&link.node))
    {
        link.reviewed = Some(hashes.nodes[&places[&link.node].id]);
    }
    Ok(())
}

/// Accepts every link with a `TR-002` or `TR-003` finding and no `TR-004` or `TR-005` finding,
/// and gives the nodes whose links it accepted, in the order of the links.
pub fn accept_stale(
    trace: &mut Trace,
    document: &Document,
    hashes: &DocumentHashes,
    files: &impl TestFiles,
) -> Vec<Key> {
    let reconciled = reconcile(document, hashes, trace, files);
    let rules = |key: Key| {
        reconciled
            .findings
            .iter()
            .filter(move |finding| finding.node == key)
            .map(|finding| finding.rule)
    };
    let stale: Vec<Key> = trace
        .links
        .iter()
        .map(|link| link.node)
        .filter(|key| {
            rules(*key).any(|rule| matches!(rule, TraceRule::Tr002 | TraceRule::Tr003))
                && !rules(*key).any(|rule| matches!(rule, TraceRule::Tr004 | TraceRule::Tr005))
        })
        .collect();
    accept(trace, &stale, document, hashes, files).expect("a stale link can be accepted");
    stale
}

/// The node and rule of each `TR-004` and `TR-005` finding.
fn untrue(
    trace: &Trace,
    document: &Document,
    hashes: &DocumentHashes,
    files: &impl TestFiles,
) -> Vec<(Key, TraceRule)> {
    reconcile(document, hashes, trace, files)
        .findings
        .into_iter()
        .filter(|finding| matches!(finding.rule, TraceRule::Tr004 | TraceRule::Tr005))
        .map(|finding| (finding.node, finding.rule))
        .collect()
}

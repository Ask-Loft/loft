//! Editing a document held in memory by key: finding a node or a decision, detaching a node with
//! every node beneath it, inserting such a subtree under a parent at a position, and moving a
//! node to another parent or position. Each edit is checked whole before it changes anything,
//! so a refused edit leaves the document as it was. No edit sets or changes a slug: the codec
//! writes each node where its parent and its slug place it.
//!
//! A decision is found by its key but stands in no parent's list, so no edit detaches, inserts
//! or moves one; nor does an edit change one.

mod place;
mod subtree;

use std::collections::BTreeSet;

pub use subtree::{OwnedNode, Subtree};

use crate::decision::Decision;
use crate::document::Document;
use crate::identity::Key;
use crate::ids::{ComponentId, NodeId};
use crate::kind::{NodeKind, ParentKind};
use crate::node::Node;
use place::{ListMut, Step, length, path};
use subtree::{ids_beneath, sourced_in};

/// What a key finds in a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found<'a> {
    Node(Node<'a>),
    Decision(&'a Decision),
}

/// What a node is inserted or moved under: the document's root, which holds the Products and the
/// root Components, or a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Parent {
    Root,
    Node(NodeId),
}

impl Parent {
    #[must_use]
    pub const fn kind(self) -> ParentKind {
        match self {
            Self::Root => ParentKind::Root,
            Self::Node(id) => ParentKind::Node(id.kind()),
        }
    }
}

/// Why the document cannot take an edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the document holds no {0}")]
    UnknownNode(NodeId),
    #[error("the document holds no {0} to stand under")]
    UnknownParent(NodeId),
    #[error("a {kind} cannot stand under {parent}")]
    Containment { kind: NodeKind, parent: ParentKind },
    #[error("key {0} is already carried by the document")]
    KeyHeld(Key),
    #[error("position {position} is beyond the end of a list of {length}")]
    Position { position: usize, length: usize },
    #[error("{node} cannot be moved beneath itself, under {parent}")]
    BeneathItself { node: NodeId, parent: NodeId },
    #[error("a Component without a slug cannot stand at the root")]
    NoSlug(Option<ComponentId>),
}

/// An insert the document refused, with the subtree it was given.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{refusal}")]
pub struct InsertRefused {
    pub refusal: Refusal,
    pub subtree: Subtree,
}

/// The node or decision `key` finds in `document`; none when it carries neither. Finding
/// changes nothing.
#[must_use]
pub fn find(document: &Document, key: Key) -> Option<Found<'_>> {
    if let Some(decision) = document.decisions.iter().find(|each| each.id.key() == key) {
        return Some(Found::Decision(decision));
    }
    let mut found = None;
    document.walk(&mut |node| {
        if found.is_none() && node.id().is_some_and(|id| id.key() == key) {
            found = Some(Found::Node(node));
        }
    });
    found
}

/// Takes `node`, every node beneath it and the edges they are the source of out of `document`.
/// The nodes that followed it in its parent's list close up behind it.
///
/// # Errors
///
/// [`Refusal::UnknownNode`] when the document does not hold `node`.
pub fn detach(document: &mut Document, node: NodeId) -> Result<Subtree, Refusal> {
    let steps = path(document, node).ok_or(Refusal::UnknownNode(node))?;
    let owned = take(document, &steps);
    let ids: BTreeSet<NodeId> = ids_beneath(owned.node()).into_iter().collect();
    let (edges, kept) = std::mem::take(&mut document.edges)
        .into_iter()
        .partition(|edge| sourced_in(edge, &ids));
    document.edges = kept;
    Ok(Subtree::with_edges(owned, edges))
}

/// Puts `subtree` under `parent` at `position` in its list of nodes of the subtree's kind, the
/// nodes that followed that position following it, and adds the subtree's edges.
///
/// # Errors
///
/// The refusal, with the subtree, when the document does not hold `parent`, the parent cannot
/// hold the subtree's kind, the document already carries a key of the subtree, or `position` is
/// beyond the end of the list.
pub fn insert(
    document: &mut Document,
    parent: Parent,
    position: usize,
    subtree: Subtree,
) -> Result<(), InsertRefused> {
    if let Err(refusal) = check_insert(document, parent, position, &subtree) {
        return Err(InsertRefused { refusal, subtree });
    }
    let (node, edges) = subtree.into_parts();
    let steps = parent_steps(document, parent).expect("the parent was checked");
    put(document, &steps, position, node);
    document.edges.extend(edges);
    Ok(())
}

/// Moves `node`, keeping its key and every node beneath it, to `position` in `parent`'s list of
/// nodes of its kind. Under its own parent, the position counts the list without it.
///
/// # Errors
///
/// When the document does not hold `node` or `parent`, `parent` is `node` or beneath it, the
/// parent cannot hold the node's kind, or `position` is beyond the end of the list.
pub fn move_node(
    document: &mut Document,
    node: NodeId,
    parent: Parent,
    position: usize,
) -> Result<(), Refusal> {
    let from = path(document, node).ok_or(Refusal::UnknownNode(node))?;
    let to = parent_steps(document, parent)?;
    if let Parent::Node(parent) = parent
        && to.starts_with(&from)
    {
        return Err(Refusal::BeneathItself { node, parent });
    }
    let kind = node.kind();
    check_containment(kind, parent)?;
    let siblings = &from[..from.len() - 1];
    let mut length = length(document, &to, kind);
    if to == siblings {
        length -= 1;
    }
    check_position(position, length)?;
    // A Component under a Component has no slug to stand at the root under.
    if parent == Parent::Root && kind == NodeKind::Component && !siblings.is_empty() {
        return Err(Refusal::NoSlug(Some(ComponentId::new(node.key()))));
    }
    let owned = take(document, &from);
    let to = parent_steps(document, parent).expect("the parent is not beneath the node");
    put(document, &to, position, owned);
    Ok(())
}

fn check_insert(
    document: &Document,
    parent: Parent,
    position: usize,
    subtree: &Subtree,
) -> Result<(), Refusal> {
    let steps = parent_steps(document, parent)?;
    let kind = subtree.kind();
    check_containment(kind, parent)?;
    check_keys(document, subtree)?;
    check_position(position, length(document, &steps, kind))?;
    match subtree.owned() {
        OwnedNode::Component(component) if parent == Parent::Root => {
            Err(Refusal::NoSlug(component.id))
        }
        _ => Ok(()),
    }
}

fn check_containment(kind: NodeKind, parent: Parent) -> Result<(), Refusal> {
    if parent.kind().may_contain(kind) {
        Ok(())
    } else {
        Err(Refusal::Containment {
            kind,
            parent: parent.kind(),
        })
    }
}

const fn check_position(position: usize, length: usize) -> Result<(), Refusal> {
    if position > length {
        Err(Refusal::Position { position, length })
    } else {
        Ok(())
    }
}

/// Refuses a subtree that carries a key the document carries, or one key twice.
fn check_keys(document: &Document, subtree: &Subtree) -> Result<(), Refusal> {
    let (index, _, _) = crate::index::Index::build(document);
    let mut seen = BTreeSet::new();
    for id in subtree.ids() {
        let key = id.key();
        if index.carries(key) || !seen.insert(key) {
            return Err(Refusal::KeyHeld(key));
        }
    }
    Ok(())
}

/// The steps to `parent`; none for the root.
fn parent_steps(document: &Document, parent: Parent) -> Result<Vec<Step>, Refusal> {
    match parent {
        Parent::Root => Ok(Vec::new()),
        Parent::Node(id) => path(document, id).ok_or(Refusal::UnknownParent(id)),
    }
}

/// Takes the node the steps lead to out of its parent's list.
fn take(document: &mut Document, steps: &[Step]) -> OwnedNode {
    let ((kind, position), parent) = steps.split_last().expect("a node is beneath the root");
    ListMut::under(document, parent, *kind)
        .expect("the steps lead to a list")
        .remove(*position)
}

/// Puts `node` at `position` in the list of its kind under the node the steps lead to.
fn put(document: &mut Document, steps: &[Step], position: usize, node: OwnedNode) {
    let kind = node.node().kind();
    let list = ListMut::under(document, steps, kind).expect("the parent holds the kind");
    let taken = list.insert(position, node);
    assert!(
        taken,
        "the parent's list of a kind takes a node of that kind"
    );
}

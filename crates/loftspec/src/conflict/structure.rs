//! The settlement of a conflict over `record`, `parent` or `position`, made on the document: a
//! record kept or deleted, moved under another parent, or moved within its list.

use super::settle::SettleError;
use super::{Conflict, ConflictOwner, ConflictValue};
use crate::codec::schema::field;
use crate::decision::Decision;
use crate::document::Document;
use crate::edit::{self, Parent};
use crate::identity::Key;
use crate::ids::{ElementId, NodeId};
use crate::node::Node;

/// Settles `conflict` by the side's value `side`.
///
/// # Errors
///
/// As [`super::settle`] refuses a settlement of a conflict over `record`, `parent` or
/// `position`.
pub(super) fn settle(
    document: &mut Document,
    conflict: &Conflict,
    side: &ConflictValue,
) -> Result<(), SettleError> {
    let owner = conflict.owner;
    match (conflict.field.as_str(), side) {
        (field::RECORD, ConflictValue::Deleted) => delete(document, owner),
        (field::RECORD, _) => Ok(()),
        (field::PARENT, ConflictValue::Scalar(parent)) => {
            reparent(document, owner, key(parent, owner)?)
        }
        (field::POSITION, ConflictValue::Scalar(anchor)) => {
            let anchor = (anchor != "first")
                .then(|| key(anchor, owner))
                .transpose()?;
            reposition(document, owner, anchor)
        }
        _ => Err(SettleError::Unsettleable(owner)),
    }
}

/// The key a side names; a side that names no key cannot be settled.
fn key(text: &str, owner: ConflictOwner) -> Result<Key, SettleError> {
    Key::parse(text).map_err(|_| SettleError::Unsettleable(owner))
}

/// Deletes the record `owner` names, with every record beneath it and the edges they write,
/// and departs the binding of each element it takes out.
fn delete(document: &mut Document, owner: ConflictOwner) -> Result<(), SettleError> {
    let missing = || SettleError::Unsettleable(owner);
    let gone: Vec<ElementId> = match owner {
        ConflictOwner::Element(ElementId::Node(node)) => {
            let subtree = edit::detach(document, node).map_err(SettleError::Edit)?;
            subtree.ids().into_iter().map(ElementId::Node).collect()
        }
        ConflictOwner::Element(ElementId::Decision(decision)) => {
            let at = (document.decisions.iter())
                .position(|each| each.id == decision)
                .ok_or_else(missing)?;
            document.decisions.remove(at);
            let gone = ElementId::Decision(decision);
            document.edges.retain(|edge| edge.source() != gone);
            vec![gone]
        }
        ConflictOwner::Option(_) | ConflictOwner::Driver(_) | ConflictOwner::Consequence(_) => {
            let removed = document
                .decisions
                .iter_mut()
                .any(|decision| take_part(decision, owner));
            return if removed { Ok(()) } else { Err(missing()) };
        }
        ConflictOwner::Tag(tag) => retain(&mut document.tags, |entry| entry.id != tag, owner)?,
        ConflictOwner::Category(category) => retain(
            &mut document.categories,
            |entry| entry.id != category,
            owner,
        )?,
        ConflictOwner::Term(term) => retain(&mut document.terms, |entry| entry.id != term, owner)?,
        ConflictOwner::Manifest | ConflictOwner::Labels => return Err(missing()),
    };
    for element in gone {
        if document.bindings.label_of(element).is_some() {
            document
                .bindings
                .depart(element)
                .expect("a bound element departs");
        }
    }
    Ok(())
}

/// Keeps the entries `kept` accepts, refusing where it accepts every one.
fn retain<T>(
    entries: &mut Vec<T>,
    kept: impl Fn(&T) -> bool,
    owner: ConflictOwner,
) -> Result<Vec<ElementId>, SettleError> {
    let before = entries.len();
    entries.retain(kept);
    if entries.len() == before {
        return Err(SettleError::Unsettleable(owner));
    }
    Ok(Vec::new())
}

/// Takes out of `decision` the option, driver or consequence `owner` names; whether it held it.
fn take_part(decision: &mut Decision, owner: ConflictOwner) -> bool {
    let before = decision.options.len() + decision.drivers.len() + decision.consequences.len();
    match owner {
        ConflictOwner::Option(id) => decision.options.retain(|each| each.id != id),
        ConflictOwner::Driver(id) => decision.drivers.retain(|each| each.id != id),
        ConflictOwner::Consequence(id) => decision.consequences.retain(|each| each.id != id),
        _ => {}
    }
    before != decision.options.len() + decision.drivers.len() + decision.consequences.len()
}

/// Moves the record `owner` names under the record keyed `parent`, after the records of its
/// kind there; a record already there stays where it is.
fn reparent(document: &mut Document, owner: ConflictOwner, parent: Key) -> Result<(), SettleError> {
    let ConflictOwner::Element(ElementId::Node(node)) = owner else {
        return move_part(document, owner, parent);
    };
    let target = match edit::find(document, parent) {
        Some(edit::Found::Node(found)) => found.id().ok_or(SettleError::NoRecord(parent))?,
        _ => return Err(SettleError::NoRecord(parent)),
    };
    let (present, _) = siblings(document, node).ok_or(SettleError::Unsettleable(owner))?;
    let target = Parent::Node(target);
    if present == target {
        return Ok(());
    }
    let length = kind_count(document, &target, node);
    edit::move_node(document, node, target, length).map_err(SettleError::Edit)
}

/// The number of nodes of `node`'s kind under `parent`.
fn kind_count(document: &Document, parent: &Parent, node: NodeId) -> usize {
    let Parent::Node(parent) = parent else {
        return 0;
    };
    let mut count = 0;
    document.walk(&mut |each| {
        if each.id() == Some(*parent) {
            count = each
                .children()
                .iter()
                .filter(|child| child.kind() == node.kind())
                .count();
        }
    });
    count
}

/// Moves an option, a driver or a consequence to the end of its list in the decision keyed
/// `parent`.
fn move_part(
    document: &mut Document,
    owner: ConflictOwner,
    parent: Key,
) -> Result<(), SettleError> {
    if !matches!(
        owner,
        ConflictOwner::Option(_) | ConflictOwner::Driver(_) | ConflictOwner::Consequence(_)
    ) {
        return Err(SettleError::Unsettleable(owner));
    }
    let to = (document.decisions.iter())
        .position(|decision| decision.id.key() == parent)
        .ok_or(SettleError::NoRecord(parent))?;
    let from = (document.decisions.iter())
        .position(|decision| holds_part(decision, owner))
        .ok_or(SettleError::Unsettleable(owner))?;
    if from == to {
        return Ok(());
    }
    let mut moved = document.decisions[from].clone();
    let source = &mut document.decisions[from];
    take_part(source, owner);
    keep_only(&mut moved, owner);
    let target = &mut document.decisions[to];
    target.options.extend(moved.options);
    target.drivers.extend(moved.drivers);
    target.consequences.extend(moved.consequences);
    Ok(())
}

fn holds_part(decision: &Decision, owner: ConflictOwner) -> bool {
    match owner {
        ConflictOwner::Option(id) => decision.options.iter().any(|each| each.id == id),
        ConflictOwner::Driver(id) => decision.drivers.iter().any(|each| each.id == id),
        ConflictOwner::Consequence(id) => decision.consequences.iter().any(|each| each.id == id),
        _ => false,
    }
}

/// Leaves `decision` holding only the part `owner` names.
fn keep_only(decision: &mut Decision, owner: ConflictOwner) {
    decision
        .options
        .retain(|each| owner == ConflictOwner::Option(each.id));
    decision
        .drivers
        .retain(|each| owner == ConflictOwner::Driver(each.id));
    decision
        .consequences
        .retain(|each| owner == ConflictOwner::Consequence(each.id));
}

/// Moves the record `owner` names within its list to right after the record keyed `anchor`, or
/// first for none.
fn reposition(
    document: &mut Document,
    owner: ConflictOwner,
    anchor: Option<Key>,
) -> Result<(), SettleError> {
    let ConflictOwner::Element(ElementId::Node(node)) = owner else {
        return reposition_part(document, owner, anchor);
    };
    let (parent, siblings) = siblings(document, node).ok_or(SettleError::Unsettleable(owner))?;
    let position = position_after(&siblings, anchor)?;
    edit::move_node(document, node, parent, position).map_err(SettleError::Edit)
}

/// Where a record goes in a list of `keys` that leaves it out: right after `anchor`, or first.
fn position_after(keys: &[Option<Key>], anchor: Option<Key>) -> Result<usize, SettleError> {
    let Some(anchor) = anchor else {
        return Ok(0);
    };
    keys.iter()
        .position(|each| *each == Some(anchor))
        .map(|at| at + 1)
        .ok_or(SettleError::NoRecord(anchor))
}

/// Moves an option, a driver or a consequence within its decision's list.
fn reposition_part(
    document: &mut Document,
    owner: ConflictOwner,
    anchor: Option<Key>,
) -> Result<(), SettleError> {
    let decision = (document.decisions.iter_mut())
        .find(|decision| holds_part(decision, owner))
        .ok_or(SettleError::Unsettleable(owner))?;
    match owner {
        ConflictOwner::Option(id) => reorder(
            &mut decision.options,
            |each| each.id.key(),
            id.key(),
            anchor,
        ),
        ConflictOwner::Driver(id) => reorder(
            &mut decision.drivers,
            |each| each.id.key(),
            id.key(),
            anchor,
        ),
        ConflictOwner::Consequence(id) => reorder(
            &mut decision.consequences,
            |each| each.id.key(),
            id.key(),
            anchor,
        ),
        _ => Err(SettleError::Unsettleable(owner)),
    }
}

/// Moves the entry keyed `moved` of `entries` to right after the one keyed `anchor`, or first.
fn reorder<T>(
    entries: &mut Vec<T>,
    key_of: impl Fn(&T) -> Key,
    moved: Key,
    anchor: Option<Key>,
) -> Result<(), SettleError> {
    let from = entries
        .iter()
        .position(|each| key_of(each) == moved)
        .expect("the decision holds the part");
    let entry = entries.remove(from);
    let keys: Vec<Option<Key>> = entries.iter().map(|each| Some(key_of(each))).collect();
    match position_after(&keys, anchor) {
        Ok(at) => {
            entries.insert(at, entry);
            Ok(())
        }
        Err(refusal) => {
            entries.insert(from, entry);
            Err(refusal)
        }
    }
}

/// The parent of `node` and the keys of the other nodes of its kind there, in order.
fn siblings(document: &Document, node: NodeId) -> Option<(Parent, Vec<Option<Key>>)> {
    let others = |nodes: Vec<Node<'_>>| -> Option<Vec<Option<Key>>> {
        let mut found = false;
        let keys = nodes
            .into_iter()
            .filter(|each| each.kind() == node.kind())
            .filter_map(|each| {
                let id = each.id();
                found |= id == Some(node);
                (id != Some(node)).then(|| id.map(NodeId::key))
            })
            .collect();
        found.then_some(keys)
    };
    if let Some(keys) = others(document.root_nodes().collect()) {
        return Some((Parent::ROOT, keys));
    }
    let mut held = None;
    document.walk(&mut |parent| {
        if held.is_none()
            && let Some(id) = parent.id()
            && let Some(keys) = others(parent.children())
        {
            held = Some((Parent::Node(id), keys));
        }
    });
    held
}

//! The settlement of a conflict over `record`, `parent` or `position`, made on the document: a
//! record kept or deleted, moved under another parent, or moved within its list.

use std::collections::BTreeSet;

use super::references::forget_elements;
use super::settle::SettleError;
use super::{Conflict, ConflictOwner, ConflictValue, FIRST};
use crate::codec::schema::field;
use crate::decision::{Consequence, Decision, DecisionOption, Driver};
use crate::document::Document;
use crate::edit::{self, Parent};
use crate::identity::Key;
use crate::ids::ElementId;

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
            let anchor = (anchor != FIRST).then(|| key(anchor, owner)).transpose()?;
            reposition(document, owner, anchor)
        }
        _ => Err(SettleError::Unsettleable(owner)),
    }
}

/// The key a side names; a side that names no key cannot be settled.
fn key(text: &str, owner: ConflictOwner) -> Result<Key, SettleError> {
    Key::parse(text).map_err(|_| SettleError::Unsettleable(owner))
}

/// Deletes the record `owner` names, with every record beneath it, the edges they write and the
/// conflict records of each record it takes out, and forgets what names the elements it takes
/// out, as [`forget_elements`] forgets them.
fn delete(document: &mut Document, owner: ConflictOwner) -> Result<(), SettleError> {
    let missing = || SettleError::Unsettleable(owner);
    let mut owners = BTreeSet::from([owner]);
    let gone: BTreeSet<ElementId> = match owner {
        ConflictOwner::Element(ElementId::Node(node)) => {
            let subtree = edit::detach(document, node).map_err(SettleError::Edit)?;
            subtree.ids().into_iter().map(ElementId::Node).collect()
        }
        ConflictOwner::Element(ElementId::Decision(decision)) => {
            let at = (document.decisions.iter())
                .position(|each| each.id == decision)
                .ok_or_else(missing)?;
            let removed = document.decisions.remove(at);
            let gone = ElementId::Decision(decision);
            document.edges.retain(|edge| edge.source() != gone);
            owners.extend(parts(&removed));
            BTreeSet::from([gone])
        }
        ConflictOwner::Option(_) | ConflictOwner::Driver(_) | ConflictOwner::Consequence(_) => {
            take_held(document, owner)?;
            BTreeSet::new()
        }
        ConflictOwner::Tag(tag) => {
            remove(&mut document.tags, |entry| entry.id == tag, owner)?;
            BTreeSet::new()
        }
        ConflictOwner::Category(category) => {
            remove(
                &mut document.categories,
                |entry| entry.id == category,
                owner,
            )?;
            BTreeSet::new()
        }
        ConflictOwner::Term(term) => {
            remove(&mut document.terms, |entry| entry.id == term, owner)?;
            BTreeSet::new()
        }
        ConflictOwner::Manifest | ConflictOwner::Labels => return Err(missing()),
    };
    owners.extend(gone.iter().copied().map(ConflictOwner::Element));
    document
        .conflicts
        .retain(|conflict| !owners.contains(&conflict.owner));
    forget_elements(document, &gone);
    Ok(())
}

/// The owners of the conflicts of a decision's options, drivers and consequences.
fn parts(decision: &Decision) -> impl Iterator<Item = ConflictOwner> + '_ {
    let options = (decision.options.iter()).map(|each| ConflictOwner::Option(each.id));
    let drivers = (decision.drivers.iter()).map(|each| ConflictOwner::Driver(each.id));
    let consequences =
        (decision.consequences.iter()).map(|each| ConflictOwner::Consequence(each.id));
    options.chain(drivers).chain(consequences)
}

/// Removes the entry `named` picks, refusing where it picks none.
fn remove<T>(
    entries: &mut Vec<T>,
    named: impl Fn(&T) -> bool,
    owner: ConflictOwner,
) -> Result<(), SettleError> {
    let at = (entries.iter())
        .position(named)
        .ok_or(SettleError::Unsettleable(owner))?;
    entries.remove(at);
    Ok(())
}

/// A decision's option, driver or consequence, taken out of its list.
enum Part {
    Option(DecisionOption),
    Driver(Driver),
    Consequence(Consequence),
}

/// Takes the part `owner` names out of `decision`, with its position in its list; none where
/// the decision does not hold it.
fn take(decision: &mut Decision, owner: ConflictOwner) -> Option<(Part, usize)> {
    fn out<T>(list: &mut Vec<T>, named: impl Fn(&T) -> bool) -> Option<(T, usize)> {
        let at = list.iter().position(named)?;
        Some((list.remove(at), at))
    }
    Some(match owner {
        ConflictOwner::Option(id) => {
            let (part, at) = out(&mut decision.options, |each| each.id == id)?;
            (Part::Option(part), at)
        }
        ConflictOwner::Driver(id) => {
            let (part, at) = out(&mut decision.drivers, |each| each.id == id)?;
            (Part::Driver(part), at)
        }
        ConflictOwner::Consequence(id) => {
            let (part, at) = out(&mut decision.consequences, |each| each.id == id)?;
            (Part::Consequence(part), at)
        }
        _ => return None,
    })
}

/// Puts `part` into its list of `decision` at `at`, or at the end for none.
fn put(decision: &mut Decision, part: Part, at: Option<usize>) {
    fn into<T>(list: &mut Vec<T>, part: T, at: Option<usize>) {
        list.insert(at.unwrap_or(list.len()), part);
    }
    match part {
        Part::Option(part) => into(&mut decision.options, part, at),
        Part::Driver(part) => into(&mut decision.drivers, part, at),
        Part::Consequence(part) => into(&mut decision.consequences, part, at),
    }
}

/// The keys of the list of `decision` that holds parts of `part`'s kind, in order.
fn keys(decision: &Decision, part: &Part) -> Vec<Key> {
    match part {
        Part::Option(_) => decision.options.iter().map(|each| each.id.key()).collect(),
        Part::Driver(_) => decision.drivers.iter().map(|each| each.id.key()).collect(),
        Part::Consequence(_) => (decision.consequences.iter())
            .map(|each| each.id.key())
            .collect(),
    }
}

/// The part `owner` names, taken out of the decision that holds it, with that decision's place
/// among the decisions and the part's position in its list.
fn take_held(
    document: &mut Document,
    owner: ConflictOwner,
) -> Result<(usize, Part, usize), SettleError> {
    (document.decisions.iter_mut().enumerate())
        .find_map(|(holder, decision)| take(decision, owner).map(|(part, at)| (holder, part, at)))
        .ok_or(SettleError::Unsettleable(owner))
}

/// Moves the record `owner` names under the record keyed `parent`, after the records of its
/// kind there; a record already there stays where it is.
fn reparent(document: &mut Document, owner: ConflictOwner, parent: Key) -> Result<(), SettleError> {
    let ConflictOwner::Element(ElementId::Node(node)) = owner else {
        return move_part(document, owner, parent);
    };
    let Some(edit::Found::Node(found)) = edit::find(document, parent) else {
        return Err(SettleError::NoRecord(parent));
    };
    let target = Parent::Node(found.id().ok_or(SettleError::NoRecord(parent))?);
    let children = found.children().into_iter();
    let length = children.filter(|child| child.kind() == node.kind()).count();
    let (present, _) = edit::siblings(document, node).ok_or(SettleError::Unsettleable(owner))?;
    if present == target {
        return Ok(());
    }
    edit::move_node(document, node, target, length).map_err(SettleError::Edit)
}

/// Moves an option, a driver or a consequence to the end of its list in the decision keyed
/// `parent`; one that decision holds already stays where it is.
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
    let (from, part, at) = take_held(document, owner)?;
    let at = (from == to).then_some(at);
    put(&mut document.decisions[to], part, at);
    Ok(())
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
    let (parent, siblings) =
        edit::siblings(document, node).ok_or(SettleError::Unsettleable(owner))?;
    let others = siblings
        .into_iter()
        .filter(|each| *each != Some(node.key()));
    let position = position_after(others, anchor)?;
    edit::move_node(document, node, parent, position).map_err(SettleError::Edit)
}

/// Where a record goes in a list of `keys` that leaves it out: right after `anchor`, or first.
fn position_after(
    keys: impl IntoIterator<Item = Option<Key>>,
    anchor: Option<Key>,
) -> Result<usize, SettleError> {
    let Some(anchor) = anchor else {
        return Ok(0);
    };
    (keys.into_iter())
        .position(|each| each == Some(anchor))
        .map(|at| at + 1)
        .ok_or(SettleError::NoRecord(anchor))
}

/// Moves an option, a driver or a consequence within its decision's list, putting it back where
/// it was when the list does not hold the anchor.
fn reposition_part(
    document: &mut Document,
    owner: ConflictOwner,
    anchor: Option<Key>,
) -> Result<(), SettleError> {
    let (holder, part, from) = take_held(document, owner)?;
    let decision = &mut document.decisions[holder];
    let position = position_after(keys(decision, &part).into_iter().map(Some), anchor);
    put(decision, part, Some(*position.as_ref().unwrap_or(&from)));
    position.map(|_| ())
}

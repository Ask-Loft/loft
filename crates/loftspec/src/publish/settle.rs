//! Settling two publishes: a publish run in a stopped merge or rebase of the publish branch. It
//! refuses an unmerged path other than the bindings and the lock, and a stop of another branch;
//! numbers the publish one past the higher of the two commits' locks; and settles the conflict
//! over `bindings` the merge driver wrote, keeping the pulled side's bindings and withdrawing
//! the other side's colliding labels. In a merge the pulled side is `theirs`; in a rebase, which
//! replays the local commits onto the pulled ones, it is `ours`.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use super::labels::elements_in_order;
use super::{Lock, Operation, Refusal, Stopped};
use crate::bindings::Bindings;
use crate::codec::field;
use crate::conflict::{ConflictOwner, ConflictValue};
use crate::document::Document;
use crate::identity::{Key, Label};
use crate::ids::ElementId;

/// The unmerged paths a settling publish resolves, from the directory that holds `spec/`.
const SETTLED: [&str; 2] = ["spec/labels.yaml", "loft.lock.json"];

/// A label withdrawn from a key because the pulled side gives it to another key, or gives the
/// key another label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdrawn {
    pub element: Key,
    pub label: Label,
}

/// Refuses an unmerged path other than the bindings and the lock, then a stop of a branch other
/// than `publish_branch`; otherwise gives the publish sequence: one past the higher of the
/// sequences of the locks at `HEAD` and at the commit being merged or replayed, or 1.
pub(super) fn preconditions(
    stopped: &Stopped,
    publish_branch: &str,
) -> Result<NonZeroU64, Refusal> {
    let unmerged: Vec<String> = stopped
        .unmerged
        .iter()
        .filter(|path| !SETTLED.contains(&path.as_str()))
        .cloned()
        .collect();
    if !unmerged.is_empty() {
        return Err(Refusal::Unmerged(unmerged));
    }
    if stopped.branch.as_deref() != Some(publish_branch) {
        return Err(Refusal::Branch {
            current: stopped.branch.clone(),
            publish: publish_branch.to_owned(),
        });
    }
    let other = match stopped.operation {
        Operation::Merge => "MERGE_HEAD",
        Operation::Rebase => "REBASE_HEAD",
    };
    let mut highest = None;
    for (revision, bytes) in [("HEAD", &stopped.head_lock), (other, &stopped.other_lock)] {
        if let Some(bytes) = bytes {
            let lock = Lock::read(bytes).map_err(|error| Refusal::Lock {
                revision,
                message: error.to_string(),
            })?;
            highest = highest.max(Some(lock.publish_sequence));
        }
    }
    Ok(highest.map_or(NonZeroU64::MIN, |sequence| sequence.saturating_add(1)))
}

/// What settling did: the labels it withdrew, and the history of each key that lost its live
/// label, which allocation rebinds before the history is attached again.
#[derive(Debug, Default)]
pub(super) struct Settlement {
    pub(super) withdrawn: Vec<Withdrawn>,
    pending: Vec<History>,
}

/// The aliases and withdrawn labels a key keeps.
#[derive(Debug)]
struct History {
    element: ElementId,
    aliases: Vec<Label>,
    withdrawn: Vec<Label>,
}

impl History {
    fn attach(&self, bindings: &mut Bindings) {
        for alias in &self.aliases {
            let added = bindings.add_alias(self.element, *alias);
            added.expect("a settled key is bound");
        }
        for withdrawn in &self.withdrawn {
            let added = bindings.add_withdrawn(self.element, *withdrawn);
            added.expect("a settled key is bound");
        }
    }
}

impl Settlement {
    /// Attaches the history of each key that allocation rebound.
    pub(super) fn reattach(&self, bindings: &mut Bindings) {
        for history in &self.pending {
            history.attach(bindings);
        }
    }
}

/// Settles the conflict over `bindings` in `document`, where it holds one, and removes it.
pub(super) fn settle(document: &mut Document, operation: Operation) -> Settlement {
    let mut settlement = Settlement::default();
    let Some(at) = document.conflicts.iter().position(|conflict| {
        conflict.owner == ConflictOwner::Labels && conflict.field.as_str() == field::BINDINGS
    }) else {
        return settlement;
    };
    let conflict = document.conflicts.remove(at);
    let (pulled, other) = match operation {
        Operation::Merge => (&conflict.theirs, &conflict.ours),
        Operation::Rebase => (&conflict.ours, &conflict.theirs),
    };
    let (pulled, other) = (sides(pulled.as_ref()), sides(other.as_ref()));
    let elements: BTreeMap<Key, ElementId> = elements_in_order(document)
        .into_iter()
        .map(|element| (element.key(), element))
        .collect();
    let disputed: BTreeSet<Key> = pulled.keys().chain(other.keys()).copied().collect();
    let held = held(&document.bindings, &disputed, &pulled);
    for key in &disputed {
        let Some(&element) = elements.get(key) else {
            continue;
        };
        document.bindings.unbind(element);
        let outcome = settled(*key, pulled.get(key), other.get(key), &held);
        settlement.add(element, outcome, &mut document.bindings);
    }
    settlement
}

impl Settlement {
    fn add(&mut self, element: ElementId, outcome: Outcome, bindings: &mut Bindings) {
        let lost = outcome.lost.iter().map(|label| Withdrawn {
            element: element.key(),
            label: *label,
        });
        self.withdrawn.extend(lost);
        let history = History {
            element,
            aliases: outcome.aliases,
            withdrawn: outcome.withdrawn,
        };
        match outcome.live {
            Some(live) => {
                let bound = bindings.bind_as_written(element, live);
                bound.expect("a disputed key was unbound");
                history.attach(bindings);
            }
            None => self.pending.push(history),
        }
    }
}

/// One side's binding of a disputed key.
struct Side {
    key: Key,
    label: Label,
    aliases: Vec<Label>,
    withdrawn: Vec<Label>,
}

impl Side {
    fn labels(&self) -> impl Iterator<Item = Label> + '_ {
        std::iter::once(self.label).chain(self.aliases.iter().copied())
    }
}

/// The bindings a side of the conflict lists, by key.
fn sides(value: Option<&ConflictValue>) -> BTreeMap<Key, Side> {
    let Some(ConflictValue::List(items)) = value else {
        return BTreeMap::new();
    };
    items
        .iter()
        .filter_map(side)
        .map(|side| (side.key, side))
        .collect()
}

fn side(value: &ConflictValue) -> Option<Side> {
    let ConflictValue::Record(fields) = value else {
        return None;
    };
    let named = |name: &str| fields.iter().find(|(each, _)| each == name).map(|(_, v)| v);
    let text = |name: &str| match named(name) {
        Some(ConflictValue::Scalar(text)) => Some(text.as_str()),
        _ => None,
    };
    let labels = |name: &str| match named(name) {
        Some(ConflictValue::List(items)) => items
            .iter()
            .filter_map(|item| match item {
                ConflictValue::Scalar(text) => Label::parse(text).ok(),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(Side {
        key: Key::parse(text(field::KEY)?).ok()?,
        label: Label::parse(text(field::LABEL)?).ok()?,
        aliases: labels(field::ALIASES),
        withdrawn: labels(field::WITHDRAWN),
    })
}

/// The key each label is held by on the pulled side: the merged bindings of the undisputed keys,
/// then the pulled side's disputed bindings.
fn held(
    bindings: &Bindings,
    disputed: &BTreeSet<Key>,
    pulled: &BTreeMap<Key, Side>,
) -> BTreeMap<Label, Key> {
    let undisputed = bindings
        .iter()
        .filter(|binding| !disputed.contains(&binding.element.key()))
        .flat_map(|binding| binding.labels().map(|label| (label, binding.element.key())));
    let disputed = pulled
        .values()
        .flat_map(|side| side.labels().map(|label| (label, side.key)));
    undisputed.chain(disputed).collect()
}

/// A disputed key's settled binding: its live label, none when it lost it, its aliases and its
/// withdrawn labels, and the labels settling withdrew from it.
struct Outcome {
    live: Option<Label>,
    aliases: Vec<Label>,
    withdrawn: Vec<Label>,
    lost: Vec<Label>,
}

/// Keeps the pulled side's binding of `key`, and of the other side's labels for it withdraws
/// each that another key holds or that the pulled side replaced, keeping the rest as aliases.
fn settled(
    key: Key,
    pulled: Option<&Side>,
    other: Option<&Side>,
    held: &BTreeMap<Label, Key>,
) -> Outcome {
    let collides = |label: Label| held.get(&label).is_some_and(|holder| *holder != key);
    let (mut live, mut aliases) = pulled.map_or((None, Vec::new()), |side| {
        (Some(side.label), side.aliases.clone())
    });
    let mut lost = Vec::new();
    if let Some(other) = other {
        if pulled.is_none() && !collides(other.label) {
            live = Some(other.label);
        }
        for label in other.labels() {
            if live == Some(label) || aliases.contains(&label) {
                continue;
            }
            if collides(label) || label == other.label {
                lost.push(label);
            } else {
                aliases.push(label);
            }
        }
    }
    let mut withdrawn: Vec<Label> = Vec::new();
    let earlier = pulled
        .into_iter()
        .chain(other)
        .flat_map(|side| &side.withdrawn);
    for label in earlier.chain(&lost) {
        if !withdrawn.contains(label) {
            withdrawn.push(*label);
        }
    }
    Outcome {
        live,
        aliases,
        withdrawn,
        lost,
    }
}

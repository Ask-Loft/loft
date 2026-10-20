//! The merge of `spec/labels.yaml` (F-32, F-60): each label counter
//! takes the higher of the two sides' values, and the bindings that two publishes gave apart
//! leave the merged bindings for one conflict record over `bindings` that lists only them. A
//! publish run in the stopped merge or rebase settles them. What a publish records in a binding
//! beside its label, its departure and its decision's state and history, merges apart from the
//! label and puts no binding in dispute.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::conflict;
use super::record::merge_own;
use super::same::{same, same_record};
use crate::codec::schema::field;
use crate::codec::{Rec, Val};
use crate::identity::Label;

/// The bindings of one side, by key.
type ByKey = BTreeMap<String, Rec>;

/// Merges the labels file, and names the labels in dispute.
pub(super) fn merge(base: Option<&Rec>, ours: &Rec, theirs: &Rec) -> (Rec, Vec<Label>) {
    let sides = [
        base.map_or_else(ByKey::new, by_key),
        by_key(ours),
        by_key(theirs),
    ];
    let labelled = sides.each_ref().map(|side| {
        let parts = side
            .iter()
            .map(|(key, binding)| (key.clone(), label_part(binding)));
        parts.collect::<ByKey>()
    });
    let (mut merged, disputed) = bindings(&labelled);
    records_apart(&sides, &mut merged, &disputed);
    // The bindings and counters are merged here; the merge of its own fields sees the rest.
    let counters = counters(ours, theirs);
    let rest = |side: &Rec| {
        let mut rest = side.clone();
        rest.fields
            .retain(|(name, _)| ![field::BINDINGS, field::COUNTERS].contains(name));
        rest
    };
    let mut record = merge_own(base.map(rest).as_ref(), &rest(ours), &rest(theirs));
    record.set(field::COUNTERS, Val::Record(counters));
    record.set(field::BINDINGS, records(merged.values()));
    if !disputed.is_empty() {
        let lists = sides
            .each_ref()
            .map(|side| records(disputed.iter().filter_map(|key| side.get(key))));
        let over_bindings = conflict(field::BINDINGS, lists.each_ref().map(Some));
        record.conflicts.push(over_bindings);
    }
    (record, labels_in_dispute(&sides, &disputed))
}

fn by_key(labels: &Rec) -> ByKey {
    labels
        .records(field::BINDINGS)
        .map(|binding| {
            let key = binding.text(field::KEY).unwrap_or_default().to_owned();
            (key, binding.clone())
        })
        .collect()
}

fn records<'a>(bindings: impl Iterator<Item = &'a Rec>) -> Val {
    Val::List(bindings.cloned().map(Val::Record).collect())
}

/// The fields a publish records in a binding beside its label.
const RECORDED: [&str; 3] = [
    field::DEPARTED,
    field::PUBLISHED_STATE,
    field::PUBLISHED_HISTORY,
];

/// A binding without the fields a publish records beside its label.
fn label_part(binding: &Rec) -> Rec {
    let mut part = binding.clone();
    part.fields.retain(|(name, _)| !RECORDED.contains(name));
    part
}

/// Gives each merged binding the fields a publish records, merged apart from its label: a field
/// only one side changed takes that side's value, a departure the two sides changed apart is
/// `true`, and a decision state or history they changed apart leaves both absent, for the next
/// publish to record. A disputed binding the merge keeps is base's, whole.
fn records_apart(sides: &[ByKey; 3], merged: &mut ByKey, disputed: &BTreeSet<String>) {
    let departed = Val::scalar("true");
    for (key, binding) in merged.iter_mut() {
        if disputed.contains(key) {
            binding.clone_from(&sides[0][key]);
            continue;
        }
        let value = |name| three_way(sides.each_ref().map(|side| side.get(key)?.get(name)));
        let (state, history) = match (
            value(field::PUBLISHED_STATE),
            value(field::PUBLISHED_HISTORY),
        ) {
            (ThreeWay::Kept(state), ThreeWay::Kept(history)) => (state, history),
            _ => (None, None),
        };
        let values = match value(field::DEPARTED) {
            ThreeWay::Kept(kept) => [kept, state, history],
            ThreeWay::Apart => [Some(&departed), state, history],
        };
        for (name, value) in RECORDED.into_iter().zip(values) {
            if let Some(value) = value {
                binding.set(name, value.clone());
            }
        }
    }
}

/// A field's value merged three ways.
enum ThreeWay<'a> {
    /// The value of the side that changed it, or of both where they changed it alike.
    Kept(Option<&'a Val>),
    /// The two sides changed it apart.
    Apart,
}

fn three_way([base, ours, theirs]: [Option<&Val>; 3]) -> ThreeWay<'_> {
    if same(ours, theirs) || same(theirs, base) {
        ThreeWay::Kept(ours)
    } else if same(ours, base) {
        ThreeWay::Kept(theirs)
    } else {
        ThreeWay::Apart
    }
}

/// Each counter at the higher of the values the two sides give it.
fn counters(ours: &Rec, theirs: &Rec) -> Rec {
    let mut counters = ours
        .record(field::COUNTERS)
        .cloned()
        .expect("a checked counters");
    let theirs = theirs.record(field::COUNTERS);
    let value = |text: Option<&str>| text.and_then(|text| text.parse::<u64>().ok()).unwrap_or(1);
    for (name, value_of_ours) in &mut counters.fields {
        let higher = value(value_of_ours.text()).max(value(theirs.and_then(|t| t.text(name))));
        *value_of_ours = Val::scalar(&higher);
    }
    counters
}

/// The merged bindings and the keys in dispute: a key both sides bound apart, then each key
/// whose changed binding gives a label that another key holds.
fn bindings([base, ours, theirs]: &[ByKey; 3]) -> (ByKey, BTreeSet<String>) {
    let keys: BTreeSet<&String> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .collect();
    let mut merged = ByKey::new();
    let mut disputed = BTreeSet::new();
    let same = |left: Option<&Rec>, right: Option<&Rec>| match (left, right) {
        (Some(left), Some(right)) => same_record(left, right),
        (None, None) => true,
        _ => false,
    };
    for key in keys {
        let [b, o, t] = [base.get(key), ours.get(key), theirs.get(key)];
        let kept = if same(o, t) || same(t, b) {
            o
        } else if same(o, b) {
            t
        } else if o.is_some() && t.is_some() {
            disputed.insert(key.clone());
            b
        } else {
            // One side removed a binding the other changed: the change stays.
            o.or(t)
        };
        if let Some(kept) = kept {
            merged.insert(key.clone(), kept.clone());
        }
    }
    settle_collisions(base, &mut merged, &mut disputed);
    (merged, disputed)
}

/// Puts in dispute each changed binding that gives a label another key holds, live or as an
/// alias, until no label is held twice: a disputed key keeps its base binding, or leaves.
fn settle_collisions(base: &ByKey, merged: &mut ByKey, disputed: &mut BTreeSet<String>) {
    loop {
        let colliding: Vec<String> = collisions(merged)
            .into_iter()
            .filter(|key| merged.get(key) != base.get(key))
            .collect();
        if colliding.is_empty() {
            return;
        }
        for key in colliding {
            disputed.insert(key.clone());
            match base.get(&key) {
                Some(binding) => merged.insert(key, binding.clone()),
                None => merged.remove(&key),
            };
        }
    }
}

/// The keys that hold, live or as an alias, a label another key holds too.
fn collisions(merged: &ByKey) -> BTreeSet<String> {
    let mut holders: BTreeMap<&str, BTreeSet<&String>> = BTreeMap::new();
    for (key, binding) in merged {
        for label in held(binding) {
            holders.entry(label).or_default().insert(key);
        }
    }
    holders
        .into_values()
        .filter(|keys| keys.len() > 1)
        .flatten()
        .cloned()
        .collect()
}

/// The labels a binding holds: its live label and its aliases, never its withdrawn labels.
fn held(binding: &Rec) -> impl Iterator<Item = &str> {
    binding
        .text(field::LABEL)
        .into_iter()
        .chain(binding.list(field::ALIASES).iter().filter_map(Val::text))
}

/// The labels the two sides gave the disputed keys that base did not.
fn labels_in_dispute([base, ours, theirs]: &[ByKey; 3], disputed: &BTreeSet<String>) -> Vec<Label> {
    let mut labels = BTreeSet::new();
    for key in disputed {
        let before: BTreeSet<&str> = base.get(key).map(|b| held(b).collect()).unwrap_or_default();
        for side in [ours, theirs] {
            let given = side.get(key).into_iter().flat_map(held);
            labels.extend(
                given
                    .filter(|label| !before.contains(label))
                    .filter_map(|label| Label::parse(label).ok()),
            );
        }
    }
    labels.into_iter().collect()
}

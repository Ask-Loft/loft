//! The merge of `spec/labels.yaml` (D-11 F-32, the labels of two publishes): each label counter
//! takes the higher of the two sides' values, and the bindings that two publishes gave apart
//! leave the merged bindings for one conflict record over `bindings` that lists only them. A
//! publish run in the stopped merge or rebase settles them.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::{conflict, push};
use super::record::merge_host;
use super::same::same_record;
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
    let (merged, disputed) = bindings(&sides);
    // The bindings and counters are merged here; the host merge sees the rest of the record.
    let counters = counters(ours, theirs);
    let rest = |side: &Rec| {
        let mut rest = Rec::new(side.kind);
        rest.fields = side
            .fields
            .iter()
            .filter(|(name, _)| ![field::BINDINGS, field::COUNTERS].contains(name))
            .cloned()
            .collect();
        rest
    };
    let mut record = merge_host(base.map(rest).as_ref(), &rest(ours), &rest(theirs));
    record.set(field::COUNTERS, Val::Record(counters));
    record.set(field::BINDINGS, records(merged.values()));
    if !disputed.is_empty() {
        let lists = sides
            .each_ref()
            .map(|side| records(disputed.iter().filter_map(|key| side.get(key))));
        push(
            &mut record,
            conflict(field::BINDINGS, lists.each_ref().map(Some)),
        );
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

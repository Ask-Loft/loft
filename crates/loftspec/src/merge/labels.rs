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
use super::three_way;
use crate::codec::schema::field;
use crate::codec::{Rec, Val};
use crate::identity::Label;

/// One side's binding: the part a publish gives with the label, and the whole binding.
struct Binding<'a> {
    label: Rec,
    whole: &'a Rec,
}

/// The bindings of one side, by key.
type Side<'a> = BTreeMap<&'a str, Binding<'a>>;

/// The label parts of the merged bindings, by key.
type Merged<'a> = BTreeMap<&'a str, &'a Rec>;

/// Merges the labels file, and names the labels in dispute.
pub(super) fn merge(base: Option<&Rec>, ours: &Rec, theirs: &Rec) -> (Rec, Vec<Label>) {
    let sides = [
        base.map(by_key).unwrap_or_default(),
        by_key(ours),
        by_key(theirs),
    ];
    let (merged, disputed) = bindings(&sides);
    let merged = with_records(&sides, &merged, &disputed);
    // The bindings and counters are merged here; the merge of its own fields sees the rest.
    let counters = counters(ours, theirs);
    let rest = |side: &Rec| without(side, &[field::BINDINGS, field::COUNTERS]);
    let mut record = merge_own(base.map(rest).as_ref(), &rest(ours), &rest(theirs));
    record.set(field::COUNTERS, Val::Record(counters));
    record.set(field::BINDINGS, records(merged.into_iter()));
    if !disputed.is_empty() {
        let lists = sides.each_ref().map(|side| {
            let wholes = disputed.iter().filter_map(|key| side.get(key));
            records(wholes.map(|binding| binding.whole.clone()))
        });
        let over_bindings = conflict(field::BINDINGS, lists.each_ref().map(Some));
        record.conflicts.push(over_bindings);
    }
    (record, labels_in_dispute(&sides, &disputed))
}

/// A side's bindings by key, each split once into its label part.
fn by_key(labels: &Rec) -> Side<'_> {
    labels
        .records(field::BINDINGS)
        .map(|whole| {
            let key = whole.text(field::KEY).unwrap_or_default();
            let label = without(whole, &RECORDED);
            (key, Binding { label, whole })
        })
        .collect()
}

fn records(bindings: impl Iterator<Item = Rec>) -> Val {
    Val::List(bindings.map(Val::Record).collect())
}

/// The fields a publish records in a binding beside its label.
const RECORDED: [&str; 3] = [
    field::DEPARTED,
    field::PUBLISHED_STATE,
    field::PUBLISHED_HISTORY,
];

/// A record without the fields `names`, cloning only the rest.
fn without(record: &Rec, names: &[&str]) -> Rec {
    Rec {
        kind: record.kind,
        position: record.position,
        fields: (record.fields.iter())
            .filter(|(name, _)| !names.contains(name))
            .cloned()
            .collect(),
        conflicts: record.conflicts.clone(),
    }
}

/// Each merged binding with the fields a publish records, merged apart from its label: a field
/// only one side changed takes that side's value, and a decision state or history they changed
/// apart leaves both absent, for the next publish to record. A departure, set or absent, cannot
/// change apart. A disputed binding the merge keeps is base's, whole.
fn with_records(sides: &[Side<'_>; 3], merged: &Merged<'_>, disputed: &BTreeSet<&str>) -> Vec<Rec> {
    let alike = |left: &_, right: &_| same(*left, *right);
    let with = |key: &str, label: &Rec| {
        if disputed.contains(key) {
            return sides[0][key].whole.clone();
        }
        let value = |name| {
            let [base, ours, theirs] = sides.each_ref().map(|side| side.get(key)?.whole.get(name));
            three_way(&base, ours, theirs, alike)
        };
        let (state, history) = match (
            value(field::PUBLISHED_STATE),
            value(field::PUBLISHED_HISTORY),
        ) {
            (Some(state), Some(history)) => (state, history),
            _ => (None, None),
        };
        let mut binding = label.clone();
        let values = [value(field::DEPARTED).flatten(), state, history];
        for (name, value) in RECORDED.into_iter().zip(values) {
            if let Some(value) = value {
                binding.set(name, value.clone());
            }
        }
        binding
    };
    merged.iter().map(|(key, label)| with(key, label)).collect()
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

/// The label parts of the merged bindings and the keys in dispute: a key both sides bound apart,
/// then each key whose changed binding gives a label that another key holds.
fn bindings<'a>(sides: &'a [Side<'a>; 3]) -> (Merged<'a>, BTreeSet<&'a str>) {
    let [base, ours, theirs] = sides;
    let keys: BTreeSet<&str> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .copied()
        .collect();
    let mut merged = Merged::new();
    let mut disputed = BTreeSet::new();
    for key in keys {
        let [b, o, t] = sides
            .each_ref()
            .map(|side| side.get(key).map(|binding| &binding.label));
        let kept = match three_way(&b, o, t, |left, right| same_binding(*left, *right)) {
            Some(kept) => kept,
            None if o.is_some() && t.is_some() => {
                disputed.insert(key);
                b
            }
            // One side removed a binding the other changed: the change stays.
            None => o.or(t),
        };
        if let Some(kept) = kept {
            merged.insert(key, kept);
        }
    }
    settle_collisions(base, &mut merged, &mut disputed);
    (merged, disputed)
}

/// Whether two sides bind a key alike, or both leave it unbound.
fn same_binding(left: Option<&Rec>, right: Option<&Rec>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => same_record(left, right),
        (None, None) => true,
        _ => false,
    }
}

/// Puts in dispute each changed binding that gives a label another key holds, live or as an
/// alias, until no label is held twice: a disputed key keeps its base binding, or leaves.
fn settle_collisions<'a>(
    base: &'a Side<'a>,
    merged: &mut Merged<'a>,
    disputed: &mut BTreeSet<&'a str>,
) {
    loop {
        let base_label = |key: &str| base.get(key).map(|binding| &binding.label);
        let colliding: Vec<&str> = collisions(merged)
            .into_iter()
            .filter(|key| merged.get(key).copied() != base_label(key))
            .collect();
        if colliding.is_empty() {
            return;
        }
        for key in colliding {
            disputed.insert(key);
            match base_label(key) {
                Some(binding) => merged.insert(key, binding),
                None => merged.remove(key),
            };
        }
    }
}

/// The keys that hold, live or as an alias, a label another key holds too.
fn collisions<'a>(merged: &Merged<'a>) -> BTreeSet<&'a str> {
    let mut holders: BTreeMap<&str, BTreeSet<&'a str>> = BTreeMap::new();
    for (key, binding) in merged {
        for label in held(binding) {
            holders.entry(label).or_default().insert(key);
        }
    }
    holders
        .into_values()
        .filter(|keys| keys.len() > 1)
        .flatten()
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
fn labels_in_dispute(
    [base, ours, theirs]: &[Side<'_>; 3],
    disputed: &BTreeSet<&str>,
) -> Vec<Label> {
    let mut labels = BTreeSet::new();
    for key in disputed {
        let before: BTreeSet<&str> = (base.get(key))
            .map(|binding| held(&binding.label).collect())
            .unwrap_or_default();
        for side in [ours, theirs] {
            let given = side
                .get(key)
                .into_iter()
                .flat_map(|binding| held(&binding.label));
            labels.extend(
                given
                    .filter(|label| !before.contains(label))
                    .filter_map(|label| Label::parse(label).ok()),
            );
        }
    }
    labels.into_iter().collect()
}

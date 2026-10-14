//! A version of a file read as its keyed records: for each key its own fields, its place and
//! its subtree, which the merge properties compare across base, ours, theirs and the result.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::codec::schema::Kind;

use super::edit_tree::{ENTRIES, Top, Y, list_kind, node, read, records};

/// The parent's key (none at the top of the file) and the list field a record sits in.
pub type Place = (Option<String>, &'static str);

/// One keyed record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyed {
    pub kind: Kind,
    pub place: Place,
    /// Its fields without the lists of keyed records, conflicts included.
    pub own: Y,
    /// The record with everything in it.
    pub subtree: Y,
}

/// A version of a file by key.
#[derive(Debug, Clone, Default)]
pub struct Flat {
    pub keyed: BTreeMap<String, Keyed>,
    /// Each list of keyed records, as its keys in order.
    pub lists: BTreeMap<Place, Vec<String>>,
    /// Each key carried more than once.
    pub twice: BTreeSet<String>,
}

fn own(record: &Y, kind: Kind) -> Y {
    let mut own = record.clone();
    if let Y::Map(fields) = &mut own {
        fields.retain(|(name, _)| list_kind(Some(kind), name).is_none());
    }
    own
}

/// The version `bytes` of a file holding `top`, by key; an empty version holds nothing.
pub fn flatten(top: Top, bytes: &[u8]) -> Flat {
    if bytes.is_empty() {
        return Flat::default();
    }
    let root = read(top, bytes);
    let found = records(top, &root);
    let key_at: BTreeMap<_, _> = found
        .iter()
        .map(|at| (at.path.clone(), at.key.clone()))
        .collect();
    let mut flat = Flat::default();
    for at in found {
        let place = match at.path.split_last() {
            None => (None, ""),
            Some((last, parent)) if parent.is_empty() && top == Top::Registry => (None, last.0),
            Some((last, parent)) => (key_at.get(parent).cloned(), last.0),
        };
        let subtree = node(&root, &at.path).clone();
        if !place.1.is_empty() {
            flat.lists
                .entry(place.clone())
                .or_default()
                .push(at.key.clone());
        }
        let keyed = Keyed {
            kind: at.kind,
            place,
            own: own(&subtree, at.kind),
            subtree,
        };
        if flat.keyed.insert(at.key.clone(), keyed).is_some() {
            flat.twice.insert(at.key);
        }
    }
    flat
}

impl Flat {
    pub fn keys(&self) -> BTreeSet<&str> {
        self.keyed.keys().map(String::as_str).collect()
    }

    pub fn has(&self, key: &str) -> bool {
        self.keyed.contains_key(key)
    }

    /// The keys of every record inside the record keyed `key`.
    pub fn beneath(&self, key: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut open = vec![key.to_owned()];
        while let Some(parent) = open.pop() {
            let children: Vec<String> = self
                .keyed
                .iter()
                .filter(|(_, keyed)| keyed.place.0.as_deref() == Some(&parent))
                .map(|(child, _)| child.clone())
                .collect();
            open.extend(
                children
                    .into_iter()
                    .filter(|child| found.insert(child.clone())),
            );
        }
        found
    }
}

/// Whether the record keyed `key` is the same in `side` as in `base`: its own fields, its place
/// and, in a list in authored order, its order among the records of the list that both hold.
fn unchanged(base: &Flat, side: &Flat, key: &str) -> bool {
    let (Some(before), Some(after)) = (base.keyed.get(key), side.keyed.get(key)) else {
        return false;
    };
    if before.own != after.own || before.place != after.place {
        return false;
    }
    // A registry is sorted by name; its order is no one's edit.
    if before.place.1 == ENTRIES {
        return true;
    }
    let list = |flat: &Flat| flat.lists.get(&before.place).cloned().unwrap_or_default();
    let (before_list, after_list) = (list(base), list(side));
    let common = |list: &[String], other: &[String]| -> Vec<String> {
        list.iter()
            .filter(|key| other.contains(key))
            .cloned()
            .collect()
    };
    common(&before_list, &after_list) == common(&after_list, &before_list)
}

/// How the side that kept a record the other side deleted left it: untouched, beneath it
/// included; untouched but for records beneath it that it deleted; or otherwise changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    Untouched,
    DeletedBeneath,
    Changed,
}

fn kept(base: &Flat, side: &Flat, key: &str) -> Kept {
    if !unchanged(base, side, key) {
        return Kept::Changed;
    }
    let now = side.beneath(key);
    if !now.iter().all(|inner| unchanged(base, side, inner)) {
        return Kept::Changed;
    }
    let gone: Vec<String> = base.beneath(key).difference(&now).cloned().collect();
    if gone.is_empty() {
        Kept::Untouched
    } else if gone.iter().all(|inner| !side.has(inner)) {
        Kept::DeletedBeneath
    } else {
        Kept::Changed
    }
}

/// Each record of base one side deleted where its deletion starts, with how the other side
/// kept it. Where a deletion starts, the record's parent on the keeping side is on the deleting
/// side too, or it has none; a record beneath one the merge keeps for the keeping side's change
/// comes with that record, so only where a deletion starts is a record bound to go.
pub fn deletions(base: &Flat, ours: &Flat, theirs: &Flat) -> Vec<(String, Kept)> {
    let mut found = Vec::new();
    for key in base.keyed.keys() {
        let (deleting, keeping) = match (ours.has(key), theirs.has(key)) {
            (false, true) => (ours, theirs),
            (true, false) => (theirs, ours),
            _ => continue,
        };
        let parent = &keeping.keyed[key].place.0;
        if parent.as_ref().is_none_or(|parent| deleting.has(parent)) {
            found.push((key.clone(), kept(base, keeping, key)));
        }
    }
    found
}

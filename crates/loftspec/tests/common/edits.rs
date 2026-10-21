//! The edit generator of the merge properties: random structural edits of one file of the
//! Format, made to its YAML tree and written back in canonical layout. Each edit names its
//! targets by an index into the candidates it finds when it is applied, so a sequence of edits
//! applies to any tree and shrinks as proptest shrinks the indices.

use loftspec::codec::schema::{Kind, field};
use proptest::prelude::*;
use proptest::sample::Index;

use super::edit_tree::{At, ENTRIES, Top, Y, find, node, node_mut, records};
use super::edit_values::{
    TAG_NAMES, TEXTS, conflict_targets, insert_slots, move_slots, pool_key, record_of,
    sort_registry, text_targets,
};

/// Which version of the file an edit makes, which fixes the keys and names it draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Base,
    Ours,
    Theirs,
}

impl Side {
    pub fn word(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Ours => "ours",
            Self::Theirs => "theirs",
        }
    }
}

/// Where an inserted record's key comes from: the side's own pool, or a pool both sides draw
/// from, so both may add the same key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pool {
    Own,
    Shared,
}

#[derive(Debug, Clone)]
pub enum TagChange {
    Add(usize),
    Drop(Index),
    Reorder,
}

#[derive(Debug, Clone)]
pub enum Edit {
    /// Replace a name, a body, a definition or a statement's text.
    Text { target: Index, text: usize },
    /// Delete a keyed record and everything in it.
    Delete { record: Index },
    /// Insert a new keyed record of the kind a list holds.
    Insert {
        slot: Index,
        at: Index,
        pool: Pool,
        n: usize,
        text: usize,
    },
    /// Move a keyed record within its list or to another list of the same field.
    Move {
        record: Index,
        slot: Index,
        at: Index,
    },
    /// Move a keyed record within its own list.
    Reorder { record: Index, at: Index },
    /// Add a carried conflict record to a keyed record.
    Conflict { record: Index, text: usize },
    /// Add, drop or reorder a node's tags.
    Tag { record: Index, change: TagChange },
    /// Rename a registry entry.
    Rename { record: Index, n: usize },
}

fn insertion() -> impl Strategy<Value = Edit> {
    let pool = prop_oneof![Just(Pool::Own), Just(Pool::Shared)];
    (
        any::<Index>(),
        any::<Index>(),
        pool,
        0..3usize,
        0..TEXTS.len(),
    )
        .prop_map(|(slot, at, pool, n, text)| Edit::Insert {
            slot,
            at,
            pool,
            n,
            text,
        })
}

pub fn edit() -> impl Strategy<Value = Edit> {
    let index = any::<Index>;
    let tags = prop_oneof![
        (0..TAG_NAMES.len()).prop_map(TagChange::Add),
        index().prop_map(TagChange::Drop),
        Just(TagChange::Reorder),
    ];
    prop_oneof![
        2 => (index(), 0..TEXTS.len()).prop_map(|(target, text)| Edit::Text { target, text }),
        2 => index().prop_map(|record| Edit::Delete { record }),
        3 => insertion(),
        3 => (index(), index(), index())
            .prop_map(|(record, slot, at)| Edit::Move { record, slot, at }),
        4 => (index(), index()).prop_map(|(record, at)| Edit::Reorder { record, at }),
        1 => (index(), 0..TEXTS.len()).prop_map(|(record, text)| Edit::Conflict { record, text }),
        2 => (index(), tags).prop_map(|(record, change)| Edit::Tag { record, change }),
        1 => (index(), 0..3usize).prop_map(|(record, n)| Edit::Rename { record, n }),
    ]
}

/// An edit that makes a base: mostly insertions, so that lists hold several records.
pub fn growth() -> impl Strategy<Value = Edit> {
    prop_oneof![3 => insertion(), 1 => edit()]
}

/// The file `bytes` after `edits`, made by `side`, in canonical layout.
pub fn edited(top: Top, bytes: &[u8], side: Side, edits: &[Edit]) -> Vec<u8> {
    let mut root = super::edit_tree::read(top, bytes);
    for edit in edits {
        apply(top, &mut root, side, edit);
    }
    super::edit_tree::write(top, &root)
}

fn apply(top: Top, root: &mut Y, side: Side, edit: &Edit) {
    match *edit {
        Edit::Text { target, text } => set_text(top, root, target, text),
        Edit::Delete { record } => delete(top, root, record),
        Edit::Insert {
            slot,
            at,
            pool,
            n,
            text,
        } => insert(top, root, slot, at, &pool_key(side, pool, n), text),
        Edit::Move { record, slot, at } => move_record(top, root, record, Some(slot), at),
        Edit::Reorder { record, at } => move_record(top, root, record, None, at),
        Edit::Conflict { record, text } => add_conflict(top, root, record, text),
        Edit::Tag { record, ref change } => change_tags(top, root, record, change),
        Edit::Rename { record, n } => rename(top, root, record, side, n),
    }
    if top == Top::Registry {
        sort_registry(root);
    }
}

fn pick<T>(candidates: &[T], index: Index) -> Option<&T> {
    (!candidates.is_empty()).then(|| index.get(candidates))
}

fn set_text(top: Top, root: &mut Y, target: Index, text: usize) {
    let targets = text_targets(top, root);
    let Some((at, names)) = pick(&targets, target) else {
        return;
    };
    let value = names.iter().fold(node_mut(root, &at.path), |value, name| {
        value.get_mut(name).expect("the text's field")
    });
    *value = Y::Text(TEXTS[text].to_owned());
}

/// Takes the record at `path` out of its list, dropping the list once it is empty.
fn take(top: Top, root: &mut Y, path: &[(&'static str, usize)]) -> Y {
    let (last, parent) = path.split_last().expect("a record in a list");
    let parent = node_mut(root, parent);
    let held = parent.get_mut(last.0).expect("the record's list");
    let record = held.items_mut().remove(last.1);
    if held.items().is_empty() && top != Top::Registry {
        parent.remove(last.0);
    }
    record
}

/// Puts `record` into the list `name` of the record keyed `host` (the registry for none), at
/// `at` of the places it offers.
fn put(top: Top, root: &mut Y, host: Option<&str>, name: &'static str, at: Index, record: Y) {
    let (path, kind) = match host {
        Some(key) => {
            let host = find(top, root, key).expect("the host");
            (host.path, Some(host.kind))
        }
        None => (Vec::new(), None),
    };
    let host = node_mut(root, &path);
    if host.get(name).is_none() {
        host.set(kind, name, Y::Seq(Vec::new()));
    }
    let list = host.get_mut(name).expect("the list").items_mut();
    let place = at.index(list.len() + 1);
    list.insert(place, record);
}

/// Whether a record holds a Feature Group that lists Features, whose files would be left with
/// no group.
fn lists_features(record: &Y) -> bool {
    match record {
        Y::Map(fields) => fields.iter().any(|(name, value)| {
            (*name == field::FEATURES && !value.items().is_empty()) || lists_features(value)
        }),
        Y::Seq(items) => items.iter().any(lists_features),
        Y::Plain(_) | Y::Text(_) => false,
    }
}

fn delete(top: Top, root: &mut Y, record: Index) {
    let candidates: Vec<At> = records(top, root)
        .into_iter()
        .filter(|at| !at.path.is_empty() && !lists_features(node(root, &at.path)))
        .collect();
    if let Some(at) = pick(&candidates, record) {
        let taken = take(top, root, &at.path);
        let mut gone = Vec::new();
        keys_in(&taken, &mut gone);
        unreference(root, &gone);
    }
}

/// The fields that hold edges: keys the reader resolves.
const EDGES: [&str; 3] = [field::MENTIONS, field::REALIZES, field::SCOPED_TO];

/// Every key a record and the records inside it carry.
fn keys_in(value: &Y, keys: &mut Vec<String>) {
    match value {
        Y::Map(fields) => {
            for (name, value) in fields {
                match value.text() {
                    Some(key) if *name == field::KEY => keys.push(key.to_owned()),
                    _ => keys_in(value, keys),
                }
            }
        }
        Y::Seq(items) => items.iter().for_each(|item| keys_in(item, keys)),
        Y::Plain(_) | Y::Text(_) => {}
    }
}

/// Takes every edge to a key of `gone` out of the tree, as a side that deletes a record takes
/// the references to it, and drops an edge list left empty; the file then reads.
fn unreference(value: &mut Y, gone: &[String]) {
    match value {
        Y::Map(fields) => {
            for (name, value) in fields.iter_mut() {
                if EDGES.contains(name) {
                    value.items_mut().retain(|item| !names_one_of(item, gone));
                } else {
                    unreference(value, gone);
                }
            }
            fields.retain(|(name, value)| !EDGES.contains(name) || !value.items().is_empty());
        }
        Y::Seq(items) => items.iter_mut().for_each(|item| unreference(item, gone)),
        Y::Plain(_) | Y::Text(_) => {}
    }
}

/// Whether a scalar names one of the keys `keys`.
fn names_one_of(item: &Y, keys: &[String]) -> bool {
    item.text()
        .is_some_and(|text| keys.iter().any(|key| key == text))
}

fn insert(top: Top, root: &mut Y, slot: Index, at: Index, key: &str, text: usize) {
    if find(top, root, key).is_some() {
        return;
    }
    let slots = insert_slots(top, root);
    let Some((host, name, kind)) = pick(&slots, slot) else {
        return;
    };
    let record = record_of(*kind, key, text);
    put(top, root, host.as_deref(), name, at, record);
}

/// Moves a record into the list of the same field of the record `slot` picks, or within its
/// own list for none.
fn move_record(top: Top, root: &mut Y, record: Index, slot: Option<Index>, at: Index) {
    if top == Top::Registry {
        return;
    }
    // A record alone in its list has nowhere else in it to go.
    let has_siblings = |at: &At| {
        at.path.split_last().is_some_and(|(last, parent)| {
            node(root, parent)
                .get(last.0)
                .is_some_and(|list| list.items().len() > 1)
        })
    };
    let movable: Vec<At> = records(top, root)
        .into_iter()
        .filter(|at| !at.path.is_empty() && (slot.is_some() || has_siblings(at)))
        .collect();
    let Some(moved) = pick(&movable, record).cloned() else {
        return;
    };
    let (last, parent) = moved.path.split_last().expect("a listed record");
    let host = match slot {
        Some(slot) => match pick(&move_slots(top, root, &moved), slot) {
            Some(host) => host.clone(),
            None => return,
        },
        None => node(root, parent).key().expect("a keyed parent").to_owned(),
    };
    let name = last.0;
    let value = take(top, root, &moved.path);
    put(top, root, Some(&host), name, at, value);
}

fn add_conflict(top: Top, root: &mut Y, record: Index, text: usize) {
    let targets = conflict_targets(top, root);
    let Some((at, path, current)) = pick(&targets, record).cloned() else {
        return;
    };
    let conflict = Y::Map(vec![
        (field::FIELD, Y::Plain(path)),
        (field::BASE, Y::Text(current)),
        (field::OURS, Y::Text(TEXTS[text].to_owned())),
        (
            field::THEIRS,
            Y::Text(TEXTS[(text + 1) % TEXTS.len()].to_owned()),
        ),
    ]);
    let host = node_mut(root, &at.path);
    if host.get(field::CONFLICTS).is_none() {
        host.set(Some(at.kind), field::CONFLICTS, Y::Seq(Vec::new()));
    }
    host.get_mut(field::CONFLICTS)
        .expect("the conflicts")
        .items_mut()
        .push(conflict);
}

fn change_tags(top: Top, root: &mut Y, record: Index, change: &TagChange) {
    let tagged: Vec<At> = records(top, root)
        .into_iter()
        .filter(|at| at.kind.field(field::TAGS).is_some())
        .collect();
    let Some(at) = pick(&tagged, record) else {
        return;
    };
    let host = node_mut(root, &at.path);
    let mut tags = host
        .get(field::TAGS)
        .map(|tags| tags.items().to_vec())
        .unwrap_or_default();
    match change {
        TagChange::Add(n) => {
            let tag = Y::Plain(TAG_NAMES[*n].to_owned());
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        TagChange::Drop(index) => {
            if !tags.is_empty() {
                tags.remove(index.index(tags.len()));
            }
        }
        TagChange::Reorder => {
            if !tags.is_empty() {
                tags.rotate_left(1);
            }
        }
    }
    if tags.is_empty() {
        host.remove(field::TAGS);
    } else {
        host.set(Some(at.kind), field::TAGS, Y::Seq(tags));
    }
}

fn rename(top: Top, root: &mut Y, record: Index, side: Side, n: usize) {
    if top != Top::Registry {
        return;
    }
    let entries: Vec<At> = records(top, root);
    let Some(at) = pick(&entries, record) else {
        return;
    };
    let name = format!("{}-{}", side.word(), ["one", "two", "three"][n]);
    let taken = root
        .get(ENTRIES)
        .expect("the entries")
        .items()
        .iter()
        .any(|entry| entry.get(field::NAME).and_then(Y::text) == Some(&name));
    if !taken {
        node_mut(root, &at.path).set(Some(Kind::TagEntry), field::NAME, Y::Plain(name));
    }
}

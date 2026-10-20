//! One version of a file as its keyed records (F-32): each record's own fields, with every
//! list of keyed records taken out of it, and the lists, each as its keys in order; and the
//! reverse, which builds the record tree back from merged records and merged lists. A record is
//! matched across the versions by its key alone, wherever it sits (RQ-117).

use std::collections::BTreeMap;

use super::same::same_record;
use crate::codec::schema::{Item, Kind, Order, Shape, field};
use crate::codec::{Content, Rec, Val};
use crate::identity::Key;

/// The record that holds a list of keyed records: the file's top record, which is also where a
/// registry lists its entries, or a keyed record by its key. The top is one parent on every
/// side, whatever key its record carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Parent {
    Top,
    Key(Key),
}

/// A list of keyed records: the record that holds it and its field.
pub(super) type Place = (Parent, &'static str);

/// One keyed record of a version: its own fields, and the list it sits in.
#[derive(Debug, Clone)]
pub(super) struct Node {
    pub(super) own: Rec,
    pub(super) place: Place,
}

impl Node {
    /// Whether this version changed the record from `base`: in its own fields and conflicts, or
    /// in its place.
    pub(super) fn changed_from(&self, base: &Self) -> bool {
        !same_record(&base.own, &self.own) || base.place != self.place
    }
}

/// One version of a file by key.
#[derive(Debug, Clone, Default)]
pub(super) struct Flat {
    /// The top record's own fields; none for a registry, or for an absent base.
    pub(super) top: Option<Rec>,
    pub(super) nodes: BTreeMap<Key, Node>,
    /// Each list of keyed records, as its keys in order.
    pub(super) lists: BTreeMap<Place, Vec<Key>>,
}

impl Flat {
    /// A version of a file, by key. The reader refused any input that carries a key twice.
    pub(super) fn of(content: &Content) -> Self {
        let mut flat = Self::default();
        match content {
            Content::Record(record) => {
                let top = flat.own(record, Parent::Top);
                flat.top = Some(top);
            }
            Content::Registry(entries) => {
                for entry in entries {
                    flat.node(entry, (Parent::Top, field::ENTRIES));
                }
            }
            Content::Lint(_) => {}
        }
        flat
    }

    /// The place of the record keyed `key`, where the version holds it.
    pub(super) fn place(&self, key: &Key) -> Option<Place> {
        self.nodes.get(key).map(|node| node.place)
    }

    /// The parent's key as this version names it in a `parent` conflict: a keyed record's key,
    /// or the top record's key for the top (none for a registry or a keyless top).
    pub(super) fn parent_value(&self, parent: Parent) -> Option<Val> {
        match parent {
            Parent::Key(key) => Some(Val::scalar(&key)),
            Parent::Top => self
                .top
                .as_ref()
                .filter(|top| top.kind.is_keyed())
                .and_then(|top| top.get(field::KEY).cloned()),
        }
    }

    /// Adds the keyed record `record` at `place`, after the records the list holds already, and
    /// each record inside it beneath it.
    pub(super) fn node(&mut self, record: &Rec, place: Place) {
        let key = record
            .key()
            .expect("a keyed record carries a key the reader checked");
        self.lists.entry(place).or_default().push(key);
        let own = self.own(record, Parent::Key(key));
        self.nodes.insert(key, Node { own, place });
    }

    /// The record without its lists of keyed records, each of whose records is flattened in
    /// turn as held by `parent`.
    fn own(&mut self, record: &Rec, parent: Parent) -> Rec {
        let mut own = Rec {
            kind: record.kind,
            position: record.position,
            fields: Vec::with_capacity(record.fields.len()),
            conflicts: record.conflicts.clone(),
        };
        for (name, value) in &record.fields {
            if keyed_list(record.kind, name).is_none() {
                own.fields.push((name, value.clone()));
                continue;
            }
            for child in value.records() {
                self.node(child, (parent, name));
            }
        }
        own
    }
}

/// The lists of a whole tree in which each record file's top record sits: no file holds them,
/// and the tree merger alone places records in them. Each is named apart from every field, so
/// that the merge keeps for it no order of its own: the manifest and each Feature Group list
/// their records by key, and the merge of those lists sets their order.
pub(super) const PRODUCT_FILES: &str = "product files";
pub(super) const FEATURE_FILES: &str = "feature files";
pub(super) const COMPONENT_FILES: &str = "component files";
pub(super) const DECISION_FILES: &str = "decision files";

/// The kind of record that a list of a whole tree holds in a file of its own: Products and root
/// Components and decisions at the top of the tree, and Features in a Feature Group.
fn filed(parent: Option<Kind>, name: &str) -> Option<Kind> {
    match (parent, name) {
        (None, PRODUCT_FILES) => Some(Kind::Product),
        (None, COMPONENT_FILES) => Some(Kind::RootComponent),
        (None, DECISION_FILES) => Some(Kind::Decision),
        (Some(Kind::FeatureGroup), FEATURE_FILES) => Some(Kind::Feature),
        _ => None,
    }
}

/// Whether a record of `kind` holds a list named `name` of keyed records, in its file or in the
/// files of a whole tree.
pub(super) fn holds_list(kind: Kind, name: &str) -> bool {
    keyed_list(kind, name).is_some() || filed(Some(kind), name).is_some()
}

/// The kind of keyed record a field of a record of `kind` lists; none for any other field.
fn keyed_list(kind: Kind, name: &str) -> Option<Kind> {
    match kind.field(name)?.1.shape {
        Shape::List(Item::Record(item)) if item.is_keyed() => Some(item),
        _ => None,
    }
}

/// The order a list keeps: a registry and a list sorted by key keep the canonical one, which
/// no side's edit sets (AC-790).
pub(super) fn order_of(parent: Option<Kind>, name: &str) -> Order {
    parent
        .and_then(|kind| kind.field(name))
        .map_or(Order::ByKey, |(_, field)| field.order)
}

/// Whether a list of a record of `parent` (none for a registry's top, or a whole tree's) takes a
/// record of `kind`.
pub(super) fn accepts(parent: Option<Kind>, name: &str, kind: Kind) -> bool {
    let filed = filed(parent, name) == Some(kind);
    filed
        || match parent {
            Some(parent) => keyed_list(parent, name) == Some(kind),
            None => name == field::ENTRIES,
        }
}

/// The merged records and lists, built back into a file's content: each list's records, each
/// with its own lists, from the top down. Each record sits in one list, so it is taken out of
/// `nodes` rather than copied.
pub(super) fn rebuild(
    top: Option<Rec>,
    nodes: BTreeMap<Key, Rec>,
    lists: &BTreeMap<Place, Vec<Key>>,
) -> Content {
    let mut build = Build::new(nodes, lists);
    match top {
        Some(top) => Content::Record(build.fill(top, Parent::Top)),
        None => Content::Registry(build.records(&(Parent::Top, field::ENTRIES))),
    }
}

/// Merged records, built back into record trees as merged lists hold them.
pub(super) struct Build<'a> {
    nodes: BTreeMap<Key, Rec>,
    lists: &'a BTreeMap<Place, Vec<Key>>,
}

impl<'a> Build<'a> {
    pub(super) const fn new(
        nodes: BTreeMap<Key, Rec>,
        lists: &'a BTreeMap<Place, Vec<Key>>,
    ) -> Self {
        Self { nodes, lists }
    }

    /// The record keyed `key`, with each of its lists of keyed records; none where it was
    /// taken already or the merge kept none.
    pub(super) fn take(&mut self, key: Key) -> Option<Rec> {
        let record = self.nodes.remove(&key)?;
        Some(self.fill(record, Parent::Key(key)))
    }

    /// The record keyed `key` without its lists, to change before it is taken.
    pub(super) fn get_mut(&mut self, key: Key) -> Option<&mut Rec> {
        self.nodes.get_mut(&key)
    }

    /// `record` with each of its lists of keyed records, as `parent` holds them.
    fn fill(&mut self, mut record: Rec, parent: Parent) -> Rec {
        for each in record.kind.fields() {
            if keyed_list(record.kind, each.name).is_none() {
                continue;
            }
            let records = self.records(&(parent, each.name));
            if !records.is_empty() {
                record.set(
                    each.name,
                    Val::List(records.into_iter().map(Val::Record).collect()),
                );
            }
        }
        record
    }

    fn records(&mut self, place: &Place) -> Vec<Rec> {
        let lists = self.lists;
        let keys = lists.get(place).map_or(&[][..], Vec::as_slice);
        keys.iter()
            .map(|key| {
                let record = self.nodes.remove(key).expect("a merged record in one list");
                self.fill(record, Parent::Key(*key))
            })
            .collect()
    }
}

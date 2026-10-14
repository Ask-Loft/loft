//! Where each keyed record of one file sits, and the surgery that takes a record, with every
//! record inside it, out of the file and puts it in at another parent.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::schema::{Item, Shape, field};
use crate::codec::{Rec, Val};

/// Where a keyed record sits: the key of the record that holds it, the list field it is in, and
/// the keys of the records before it in that list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Place {
    pub(super) parent: String,
    pub(super) field: &'static str,
    pub(super) before: Vec<String>,
}

impl Place {
    /// Whether two places are in one list of one parent, wherever in it.
    pub(super) fn same_parent(&self, other: &Self) -> bool {
        self.parent == other.parent && self.field == other.field
    }
}

/// One version of the file: the place of each keyed record below its top record, and the key of
/// every keyed record it carries. A key carried twice has no place: the file names no one
/// parent for it.
#[derive(Debug, Default)]
pub(super) struct Places {
    places: BTreeMap<String, Option<Place>>,
    pub(super) keys: BTreeSet<String>,
}

impl Places {
    pub(super) fn of(record: &Rec) -> Self {
        let mut places = Self::default();
        if let Some(key) = key_of(record) {
            places.keys.insert(key.to_owned());
        }
        places.walk(record);
        places
    }

    /// The place of the record that carries `key`; none where the file lacks it or carries it
    /// twice.
    pub(super) fn get(&self, key: &str) -> Option<&Place> {
        self.places.get(key).and_then(Option::as_ref)
    }

    /// Every key with a place, or carried twice.
    pub(super) fn placed(&self) -> impl Iterator<Item = &String> {
        self.places.keys()
    }

    /// Whether the record that carries `key` sits, at any depth, inside the one that carries
    /// `ancestor`.
    pub(super) fn within(&self, key: &str, ancestor: &str) -> bool {
        let mut at = self.get(key);
        // A chain longer than the keys the file carries would be a cycle, which no file holds.
        for _ in 0..=self.keys.len() {
            match at {
                Some(place) if place.parent == ancestor => return true,
                Some(place) => at = self.get(&place.parent),
                None => return false,
            }
        }
        false
    }

    fn walk(&mut self, record: &Rec) {
        let parent = key_of(record);
        for (name, value) in &record.fields {
            self.walk_field(parent, name, value);
        }
    }

    /// Places each keyed record of one field of the record that carries `parent`, and walks
    /// every record in it.
    fn walk_field(&mut self, parent: Option<&str>, name: &'static str, value: &Val) {
        let mut before = Vec::new();
        for child in value.records() {
            if let (Some(parent), Some(key)) = (parent, key_of(child)) {
                self.place(key, parent, name, &before);
                before.push(key.to_owned());
            }
            self.walk(child);
        }
    }

    /// Places the record that carries `key` in the list `field` of `parent`, after `before`.
    fn place(&mut self, key: &str, parent: &str, field: &'static str, before: &[String]) {
        let place = Place {
            parent: parent.to_owned(),
            field,
            before: before.to_vec(),
        };
        let twice = !self.keys.insert(key.to_owned());
        self.places
            .insert(key.to_owned(), (!twice).then_some(place));
    }
}

/// The key of a keyed record.
pub(super) fn key_of(record: &Rec) -> Option<&str> {
    if record.kind.is_keyed() {
        record.text(field::KEY)
    } else {
        None
    }
}

/// Takes the record that carries `key` out of the tree under `record`.
pub(super) fn take(record: &mut Rec, key: &str) -> Option<Rec> {
    for (_, value) in &mut record.fields {
        if let Val::List(items) = value
            && let Some(at) = items
                .iter()
                .position(|item| matches!(item, Val::Record(child) if key_of(child) == Some(key)))
            && let Val::Record(found) = items.remove(at)
        {
            return Some(found);
        }
        if let Some(found) = value.records_mut().find_map(|child| take(child, key)) {
            return Some(found);
        }
    }
    None
}

/// Puts `record` in at `to` in the tree under `root`: after the last of the keys `to` follows
/// that its list holds, or first. The record comes back when the tree holds no such parent, or
/// none whose list takes it.
pub(super) fn put(root: &mut Rec, record: Rec, to: &Place) -> Result<(), Rec> {
    let Some(parent) = find_mut(root, &to.parent) else {
        return Err(record);
    };
    let fits = parent
        .kind
        .field(to.field)
        .is_some_and(|(_, list)| list.shape == Shape::List(Item::Record(record.kind)));
    if !fits {
        return Err(record);
    }
    let list = parent.list_mut(to.field);
    let at = to
        .before
        .iter()
        .rev()
        .find_map(|before| {
            list.iter()
                .position(|item| matches!(item, Val::Record(each) if key_of(each) == Some(before)))
        })
        .map_or(0, |at| at + 1);
    list.insert(at, Val::Record(record));
    Ok(())
}

/// The record that carries `key`, at any depth of the tree under `record`, to change.
fn find_mut<'r>(record: &'r mut Rec, key: &str) -> Option<&'r mut Rec> {
    if key_of(record) == Some(key) {
        return Some(record);
    }
    record.children_mut().find_map(|child| find_mut(child, key))
}

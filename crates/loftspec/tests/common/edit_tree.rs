//! A file of the Format as a YAML tree without positions, which the edit generator changes and
//! the merge properties read: read with `yaml::read`, written back with `yaml::write`.

use loftspec::codec::schema::{Item, Kind, Shape, field};
use loftspec::yaml::{self, Out, Style, Value};

/// A YAML value without positions. A scalar is plain or text, as the reader gave its style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Y {
    Map(Vec<(&'static str, Y)>),
    Seq(Vec<Y>),
    Plain(String),
    Text(String),
}

/// What a file holds: one top record of a kind, or a registry of entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Top {
    Record(Kind),
    Registry,
}

/// The field a registry's entries are held under while the tree is edited; the file itself is
/// one sequence.
pub const ENTRIES: &str = "entries";

/// The field names a file may hold, each as the `'static` name the writer takes.
fn name(text: &str) -> &'static str {
    let extra = [
        field::CONFLICTS,
        field::FIELD,
        field::BASE,
        field::OURS,
        field::THEIRS,
        ENTRIES,
    ];
    Kind::ALL
        .iter()
        .flat_map(|kind| kind.fields().iter().map(|spec| spec.name))
        .chain(extra)
        .find(|known| *known == text)
        .unwrap_or_else(|| panic!("no field is named {text:?}"))
}

fn from_node(node: &yaml::Node) -> Y {
    match &node.value {
        Value::Mapping(entries) => Y::Map(
            entries
                .iter()
                .map(|entry| (name(&entry.key), from_node(&entry.value)))
                .collect(),
        ),
        Value::Sequence(items) => Y::Seq(items.iter().map(from_node).collect()),
        Value::Scalar(scalar) if scalar.style == Style::Plain => Y::Plain(scalar.text.clone()),
        Value::Scalar(scalar) => Y::Text(scalar.text.clone()),
        Value::Empty => panic!("a canonical file holds no empty value"),
    }
}

fn to_out(value: &Y) -> Out {
    match value {
        Y::Map(fields) => Out::Mapping(
            fields
                .iter()
                .map(|(name, value)| (*name, to_out(value)))
                .collect(),
        ),
        Y::Seq(items) => Out::Sequence(items.iter().map(to_out).collect()),
        Y::Plain(text) if text == "null" => Out::Null,
        Y::Plain(text) if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) => {
            Out::Integer(text.parse().expect("an integer"))
        }
        Y::Plain(text) => Out::Token(text.clone()),
        Y::Text(text) => Out::Text(text.clone()),
    }
}

/// A file's tree; a registry's entries are held under [`ENTRIES`].
pub fn read(top: Top, bytes: &[u8]) -> Y {
    let node = yaml::read(bytes).expect("the file reads as YAML");
    match top {
        Top::Record(_) => from_node(&node.expect("a record file has content")),
        Top::Registry => Y::Map(vec![(
            ENTRIES,
            node.as_ref().map_or(Y::Seq(Vec::new()), from_node),
        )]),
    }
}

/// A file's bytes, as the canonical writer lays the tree out.
pub fn write(top: Top, root: &Y) -> Vec<u8> {
    let out = match top {
        Top::Record(_) => to_out(root),
        Top::Registry => to_out(root.get(ENTRIES).expect("the registry's entries")),
    };
    yaml::write(&out).into_bytes()
}

impl Y {
    pub fn get(&self, name: &str) -> Option<&Y> {
        match self {
            Y::Map(fields) => fields
                .iter()
                .find(|(field, _)| *field == name)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Y> {
        match self {
            Y::Map(fields) => fields
                .iter_mut()
                .find(|(field, _)| *field == name)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Y::Plain(text) | Y::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn items(&self) -> &[Y] {
        match self {
            Y::Seq(items) => items,
            _ => &[],
        }
    }

    pub fn items_mut(&mut self) -> &mut Vec<Y> {
        match self {
            Y::Seq(items) => items,
            _ => panic!("not a sequence"),
        }
    }

    /// The record's key.
    pub fn key(&self) -> Option<&str> {
        self.get(field::KEY).and_then(Y::text)
    }

    /// Sets a field of a record of `kind`, where the Format's order puts it; a conflicts list
    /// goes after every other field.
    pub fn set(&mut self, kind: Option<Kind>, name: &'static str, value: Y) {
        if let Some(existing) = self.get_mut(name) {
            *existing = value;
            return;
        }
        let rank = |field: &str| {
            kind.and_then(|kind| kind.field(field))
                .map_or(usize::MAX, |(index, _)| index)
        };
        let Y::Map(fields) = self else {
            panic!("not a record")
        };
        let at = fields
            .iter()
            .position(|(field, _)| rank(field) > rank(name))
            .unwrap_or(fields.len());
        fields.insert(at, (name, value));
    }

    pub fn remove(&mut self, name: &str) {
        if let Y::Map(fields) = self {
            fields.retain(|(field, _)| *field != name);
        }
    }
}

/// The kind of the keyed records a list field of a record of `parent` holds; a registry's
/// entries are tags.
pub fn list_kind(parent: Option<Kind>, name: &str) -> Option<Kind> {
    let Some(parent) = parent else {
        return (name == ENTRIES).then_some(Kind::TagEntry);
    };
    match parent.field(name)?.1.shape {
        Shape::List(Item::Record(kind)) if kind.is_keyed() => Some(kind),
        _ => None,
    }
}

/// Where a keyed record sits: the list field and index of each step from the top.
pub type Path = Vec<(&'static str, usize)>;

/// A keyed record of a tree: where it is, its kind and its key.
#[derive(Debug, Clone)]
pub struct At {
    pub path: Path,
    pub kind: Kind,
    pub key: String,
}

/// Every keyed record of a tree in document order, the top record first.
pub fn records(top: Top, root: &Y) -> Vec<At> {
    let mut found = Vec::new();
    let kind = match top {
        Top::Record(kind) => {
            found.push(At {
                path: Vec::new(),
                kind,
                key: root.key().unwrap_or_default().to_owned(),
            });
            Some(kind)
        }
        Top::Registry => None,
    };
    collect(root, kind, &mut Vec::new(), &mut found);
    found
}

fn collect(record: &Y, kind: Option<Kind>, path: &mut Path, found: &mut Vec<At>) {
    let Y::Map(fields) = record else { return };
    for (name, value) in fields {
        let Some(child_kind) = list_kind(kind, name) else {
            continue;
        };
        for (index, child) in value.items().iter().enumerate() {
            path.push((name, index));
            found.push(At {
                path: path.clone(),
                kind: child_kind,
                key: child.key().unwrap_or_default().to_owned(),
            });
            collect(child, Some(child_kind), path, found);
            path.pop();
        }
    }
}

pub fn node<'a>(root: &'a Y, path: &[(&'static str, usize)]) -> &'a Y {
    path.iter().fold(root, |record, (name, index)| {
        &record.get(name).expect("the path's field").items()[*index]
    })
}

pub fn node_mut<'a>(root: &'a mut Y, path: &[(&'static str, usize)]) -> &'a mut Y {
    path.iter().fold(root, |record, (name, index)| {
        &mut record.get_mut(name).expect("the path's field").items_mut()[*index]
    })
}

/// Where the record keyed `key` sits.
pub fn find(top: Top, root: &Y, key: &str) -> Option<At> {
    records(top, root).into_iter().find(|at| at.key == key)
}

/// Every conflict record a tree holds.
pub fn conflicts(value: &Y) -> usize {
    match value {
        Y::Map(fields) => fields
            .iter()
            .map(|(name, value)| {
                if *name == field::CONFLICTS {
                    value.items().len()
                } else {
                    conflicts(value)
                }
            })
            .sum(),
        Y::Seq(items) => items.iter().map(conflicts).sum(),
        Y::Plain(_) | Y::Text(_) => 0,
    }
}

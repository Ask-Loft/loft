//! A record checked against its kind's schema: its fields known, none repeated, the required
//! ones present, each value of its field's shape and syntax, held in the Format's field order
//! (RQ-30, RQ-34). Formatting, reading and writing all pass through it.

use super::error::{Located, Problem};
use super::schema::{Field, Item, Kind, Scalar, Shape};
use crate::canonical::Digest;
use crate::identity::{Key, Label};
use crate::names::{CategoryName, TagName};
use crate::slug::Slug;
use crate::yaml::{self, Out, Position, Style, Value};

/// How strictly a record is read: every role but the formatter requires keys and slugs; the
/// formatter accepts their absence and supplies them (RQ-59, RQ-213).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Strict,
    Formatting,
}

/// A checked record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rec {
    pub kind: Kind,
    pub position: Position,
    /// The fields present, in the Format's order.
    pub fields: Vec<(&'static str, Val)>,
}

/// A checked field value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Val {
    Scalar { text: String, position: Position },
    Record(Rec),
    List(Vec<Val>),
}

impl Val {
    /// A scalar the writer or formatter supplies, with no position in a source file.
    #[must_use]
    pub fn scalar(text: &(impl std::fmt::Display + ?Sized)) -> Self {
        Self::Scalar {
            text: text.to_string(),
            position: Position::new(1, 1),
        }
    }

    /// The scalar's text.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Scalar { text, .. } => Some(text),
            _ => None,
        }
    }

    /// The records the value holds: itself, or a list's records.
    pub fn records(&self) -> impl Iterator<Item = &Rec> {
        let items: &[Self] = match self {
            Self::List(items) => items,
            other => std::slice::from_ref(other),
        };
        items.iter().filter_map(|item| match item {
            Self::Record(record) => Some(record),
            _ => None,
        })
    }

    /// The records the value holds, to change.
    pub fn records_mut(&mut self) -> impl Iterator<Item = &mut Rec> {
        let items: &mut [Self] = match self {
            Self::List(items) => items,
            other => std::slice::from_mut(other),
        };
        items.iter_mut().filter_map(|item| match item {
            Self::Record(record) => Some(record),
            _ => None,
        })
    }

    #[must_use]
    pub const fn position(&self) -> Option<Position> {
        match self {
            Self::Scalar { position, .. } => Some(*position),
            Self::Record(record) => Some(record.position),
            Self::List(_) => None,
        }
    }
}

impl Rec {
    #[must_use]
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            position: Position::new(1, 1),
            fields: Vec::new(),
        }
    }

    /// A field's value. Naming a field the kind does not define is a programming error.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Val> {
        debug_assert!(
            self.kind.field(name).is_some(),
            "{:?} has no field {name}",
            self.kind
        );
        self.fields
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| value)
    }

    /// A field's value, to change.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Val> {
        debug_assert!(
            self.kind.field(name).is_some(),
            "{:?} has no field {name}",
            self.kind
        );
        self.fields
            .iter_mut()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| value)
    }

    #[must_use]
    pub fn text(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(Val::text)
    }

    #[must_use]
    pub fn record(&self, name: &str) -> Option<&Self> {
        match self.get(name) {
            Some(Val::Record(record)) => Some(record),
            _ => None,
        }
    }

    /// The entries of a list field; none when the field is absent (AC-799).
    #[must_use]
    pub fn list(&self, name: &str) -> &[Val] {
        match self.get(name) {
            Some(Val::List(items)) => items,
            _ => &[],
        }
    }

    /// The records of a list field.
    pub fn records(&self, name: &str) -> impl Iterator<Item = &Self> {
        self.list(name).iter().flat_map(Val::records)
    }

    /// The entries of a list field, to change; an absent field is added empty.
    pub fn list_mut(&mut self, name: &'static str) -> &mut Vec<Val> {
        if self.get(name).is_none() {
            self.set(name, Val::List(Vec::new()));
        }
        match self.get_mut(name) {
            Some(Val::List(items)) => items,
            _ => unreachable!("{name} is a list field"),
        }
    }

    /// Sets a field, keeping the Format's order.
    pub fn set(&mut self, name: &'static str, value: Val) {
        let place = |field: &str| {
            self.kind
                .field(field)
                .expect("a field the schema defines")
                .0
        };
        let at = place(name);
        match self.fields.iter().position(|(field, _)| place(field) >= at) {
            Some(index) if self.fields[index].0 == name => self.fields[index].1 = value,
            Some(index) => self.fields.insert(index, (name, value)),
            None => self.fields.push((name, value)),
        }
    }

    /// Sets a scalar field.
    pub fn set_text(&mut self, name: &'static str, text: impl Into<String>) {
        let position = self.position;
        self.set(
            name,
            Val::Scalar {
                text: text.into(),
                position,
            },
        );
    }
}

/// Checks a node as a record of `kind`.
pub(crate) fn check(node: &yaml::Node, kind: Kind, mode: Mode) -> Result<Rec, Located> {
    let Value::Mapping(entries) = &node.value else {
        return Err(located(
            node.position,
            Problem::Expected("a mapping of fields"),
        ));
    };
    let mut record = Rec {
        kind,
        position: node.position,
        fields: Vec::with_capacity(entries.len()),
    };
    for entry in entries {
        let Some((_, field)) = kind.field(&entry.key) else {
            return Err(located(
                entry.key_position,
                Problem::UnknownField(entry.key.clone()),
            ));
        };
        let value = check_shape(&entry.value, field.shape, mode)?;
        record.fields.push((field.name, value));
    }
    for field in kind.fields() {
        if field.required && record.get(field.name).is_none() && !may_lack(kind, field, mode) {
            return Err(located(node.position, Problem::MissingField(field.name)));
        }
    }
    record
        .fields
        .sort_by_cached_key(|(name, _)| kind.field(name).map(|(at, _)| at));
    super::variants::check(&record)?;
    Ok(record)
}

/// Whether a required field may be missing in this mode: a key or a slug the formatter supplies.
fn may_lack(kind: Kind, field: &Field, mode: Mode) -> bool {
    mode == Mode::Formatting && ((field.name == "key" && kind.is_keyed()) || field.name == "slug")
}

fn located(position: Position, problem: Problem) -> Located {
    Located { position, problem }
}

fn check_shape(node: &yaml::Node, shape: Shape, mode: Mode) -> Result<Val, Located> {
    match shape {
        Shape::One(item) => check_item(node, item, mode),
        Shape::List(item) => match &node.value {
            Value::Sequence(items) => items
                .iter()
                .map(|item_node| check_item(item_node, item, mode))
                .collect::<Result<_, _>>()
                .map(Val::List),
            Value::Empty => Ok(Val::List(Vec::new())),
            _ => Err(located(node.position, Problem::Expected("a list"))),
        },
    }
}

fn check_item(node: &yaml::Node, item: Item, mode: Mode) -> Result<Val, Located> {
    match item {
        Item::Scalar(scalar) => check_scalar(node, scalar),
        Item::Record(kind) => check(node, kind, mode).map(Val::Record),
        Item::NodeRef => match &node.value {
            Value::Mapping(_) => check(node, Kind::FormerLabel, mode).map(Val::Record),
            _ => check_scalar(node, Scalar::Key),
        },
    }
}

fn check_scalar(node: &yaml::Node, scalar: Scalar) -> Result<Val, Located> {
    let Value::Scalar(value) = &node.value else {
        return Err(located(node.position, Problem::Expected(describe(scalar))));
    };
    let text = &value.text;
    let single_line = value.style != Style::Literal;
    let valid = match scalar {
        Scalar::Text => true,
        Scalar::Key => single_line && Key::parse(text).is_ok(),
        Scalar::Label => single_line && Label::parse(text).is_ok(),
        Scalar::Slug => single_line && Slug::parse(text).is_ok(),
        Scalar::Tag => single_line && TagName::parse(text).is_ok(),
        Scalar::Category => single_line && CategoryName::parse(text).is_ok(),
        Scalar::Digest => single_line && text.parse::<Digest>().is_ok(),
        Scalar::Integer => {
            value.style == Style::Plain
                && text.parse::<u64>().is_ok_and(|number| number > 0)
                && !text.starts_with('0')
        }
        Scalar::Enum(values) => single_line && values.contains(&text.as_str()),
        Scalar::Identifier => single_line && !text.is_empty() && !text.contains('\n'),
    };
    if !valid {
        return Err(located(
            node.position,
            Problem::Invalid {
                value: text.clone(),
                expected: describe(scalar),
            },
        ));
    }
    Ok(Val::Scalar {
        text: text.clone(),
        position: node.position,
    })
}

const fn describe(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Key => "a lowercase hyphenated UUID",
        Scalar::Label => "a label",
        Scalar::Slug => "a slug",
        Scalar::Tag => "a tag name",
        Scalar::Category => "a category name",
        Scalar::Digest => "a SHA-256 digest in lowercase hexadecimal",
        Scalar::Integer => "a positive integer",
        Scalar::Enum(_) => "a value the field defines",
        Scalar::Identifier => "an identifier",
        Scalar::Text => "text",
    }
}

/// The record as the writer lays it out: its fields in order, an empty list left out
/// (AC-798), each scalar in the style its shape gives.
#[must_use]
pub fn to_out(record: &Rec) -> Out {
    let fields = record
        .fields
        .iter()
        .filter(|(_, value)| !matches!(value, Val::List(items) if items.is_empty()))
        .map(|(name, value)| {
            let (_, field) = record.kind.field(name).expect("a field the schema defines");
            (field.name, value_out(value, field.shape))
        })
        .collect();
    Out::Mapping(fields)
}

fn value_out(value: &Val, shape: Shape) -> Out {
    match (shape, value) {
        (Shape::List(item), Val::List(items)) => {
            Out::Sequence(items.iter().map(|entry| item_out(entry, item)).collect())
        }
        (Shape::One(item), _) => item_out(value, item),
        (Shape::List(_), _) => unreachable!("a checked list"),
    }
}

fn item_out(value: &Val, item: Item) -> Out {
    match value {
        Val::Record(record) => to_out(record),
        Val::Scalar { text, .. } => match item {
            Item::Scalar(Scalar::Text) => Out::Text(text.clone()),
            Item::Scalar(Scalar::Integer) => Out::Integer(text.parse().expect("a checked integer")),
            _ => Out::Token(text.clone()),
        },
        Val::List(_) => unreachable!("lists do not nest"),
    }
}

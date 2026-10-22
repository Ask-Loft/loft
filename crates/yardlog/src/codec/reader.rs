//! Reading the fields of Yard Log's files, each fault at its line and column.

use std::fmt;

use loftspec::yaml::{Entry, Node, Position, Value};

use crate::keys::{Handle, Key};

/// Why a file of Yard Log does not read, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadError {
    pub position: Option<Position>,
    pub message: String,
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.position {
            Some(position) => write!(formatter, "{position}: {}", self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

impl std::error::Error for ReadError {}

pub(super) type Read<T> = Result<T, ReadError>;

pub(super) fn at(position: Position, message: impl Into<String>) -> ReadError {
    ReadError {
        position: Some(position),
        message: message.into(),
    }
}

/// A mapping's fields, as they are read; a field it does not read is refused at the end.
pub(super) struct Fields<'a> {
    entries: &'a [Entry],
    pub(super) position: Position,
    read: Vec<&'static str>,
}

impl<'a> Fields<'a> {
    pub(super) fn of(node: &'a Node) -> Read<Self> {
        match &node.value {
            Value::Mapping(entries) => Ok(Self {
                entries,
                position: node.position,
                read: Vec::new(),
            }),
            _ => Err(at(node.position, "a mapping was expected")),
        }
    }

    pub(super) fn get(&mut self, name: &'static str) -> Option<&'a Node> {
        self.read.push(name);
        self.entries
            .iter()
            .find(|entry| entry.key == name)
            .map(|entry| &entry.value)
            .filter(|node| !matches!(node.value, Value::Empty))
    }

    pub(super) fn required(&mut self, name: &'static str) -> Read<&'a Node> {
        let position = self.position;
        self.get(name)
            .ok_or_else(|| at(position, format!("the field `{name}` is missing")))
    }

    pub(super) fn text(&mut self, name: &'static str) -> Read<String> {
        scalar(self.required(name)?)
    }

    pub(super) fn optional_text(&mut self, name: &'static str) -> Read<Option<String>> {
        self.get(name).map(scalar).transpose()
    }

    pub(super) fn integer(&mut self, name: &'static str) -> Read<u64> {
        let node = self.required(name)?;
        number(node)
    }

    pub(super) fn optional_integer(&mut self, name: &'static str) -> Read<Option<u64>> {
        self.get(name).map(number).transpose()
    }

    pub(super) fn flag(&mut self, name: &'static str) -> Read<bool> {
        match self.get(name) {
            None => Ok(false),
            Some(node) => match scalar(node)?.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(at(node.position, format!("`{name}` is not true or false"))),
            },
        }
    }

    pub(super) fn key(&mut self, name: &'static str) -> Read<Key> {
        key(self.required(name)?)
    }

    pub(super) fn handle(&mut self, name: &'static str) -> Read<Handle> {
        let node = self.required(name)?;
        Handle::parse(&scalar(node)?).map_err(|error| at(node.position, error.to_string()))
    }

    /// A word of a closed set.
    pub(super) fn word<T>(&mut self, name: &'static str, parse: fn(&str) -> Option<T>) -> Read<T> {
        let node = self.required(name)?;
        let text = scalar(node)?;
        parse(&text).ok_or_else(|| {
            at(
                node.position,
                format!("`{text}` is not a value of `{name}`"),
            )
        })
    }

    pub(super) fn optional_word<T>(
        &mut self,
        name: &'static str,
        parse: fn(&str) -> Option<T>,
    ) -> Read<Option<T>> {
        if self.entries.iter().any(|entry| entry.key == name) {
            self.word(name, parse).map(Some)
        } else {
            self.read.push(name);
            Ok(None)
        }
    }

    /// A sequence's items; none where the field is left out.
    pub(super) fn list(&mut self, name: &'static str) -> Read<&'a [Node]> {
        match self.get(name) {
            None => Ok(&[]),
            Some(Node {
                value: Value::Sequence(items),
                ..
            }) => Ok(items),
            Some(node) => Err(at(node.position, format!("`{name}` is not a list"))),
        }
    }

    /// Refuses a field this mapping does not read.
    pub(super) fn finish(self) -> Read<()> {
        match self
            .entries
            .iter()
            .find(|entry| !self.read.contains(&entry.key.as_str()))
        {
            Some(entry) => Err(at(
                entry.key_position,
                format!("`{}` is not a field here", entry.key),
            )),
            None => Ok(()),
        }
    }
}

pub(super) fn scalar(node: &Node) -> Read<String> {
    match &node.value {
        Value::Scalar(scalar) => Ok(scalar.text.clone()),
        _ => Err(at(node.position, "a single value was expected")),
    }
}

fn number(node: &Node) -> Read<u64> {
    let text = scalar(node)?;
    text.parse()
        .map_err(|_| at(node.position, format!("`{text}` is not a whole number")))
}

pub(super) fn key(node: &Node) -> Read<Key> {
    Key::parse(&scalar(node)?).map_err(|error| at(node.position, error.to_string()))
}

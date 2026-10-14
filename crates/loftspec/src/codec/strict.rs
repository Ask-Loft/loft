//! The strict reading of a small YAML file beside the tree's records, such as `spec/lint.yaml`
//! and `loft.trace.yaml`: what each field must hold, each departure an error at its line and
//! column. Each file's reader records its errors in its own form and reads its own fields; the
//! checks of shape are shared here.

use super::Problem;
use crate::yaml::{Entry, Node, Position, Value};

/// A file's fields that its reader allows, found by name.
#[derive(Clone, Copy)]
pub(crate) struct Fields<'a>(&'a [Entry]);

impl<'a> Fields<'a> {
    pub(crate) fn get(self, name: &str) -> Option<&'a Entry> {
        self.0.iter().find(|entry| entry.key == name)
    }
}

/// A reader that records each error where it is, and reads with the shared checks of shape.
pub(crate) trait Strict {
    /// Records `problem` at `position`, or at the file where it has none.
    fn fail(&mut self, position: Option<Position>, problem: Problem);

    /// A mapping's entries; anything else is an error and holds none.
    fn entries<'a>(&mut self, node: &'a Node) -> &'a [Entry] {
        if let Value::Mapping(entries) = &node.value {
            return entries;
        }
        self.fail(Some(node.position), Problem::Expected("a mapping"));
        &[]
    }

    /// A sequence's items; a field written with no value holds none, and anything else is an
    /// error and holds none.
    fn items<'a>(&mut self, node: &'a Node) -> &'a [Node] {
        match &node.value {
            Value::Sequence(items) => items,
            Value::Empty => &[],
            _ => {
                self.fail(Some(node.position), Problem::Expected("a sequence"));
                &[]
            }
        }
    }

    fn scalar<'a>(&mut self, node: &'a Node) -> Option<&'a str> {
        if let Value::Scalar(scalar) = &node.value {
            return Some(&scalar.text);
        }
        self.fail(Some(node.position), Problem::Expected("a scalar"));
        None
    }

    /// A scalar that `parse` accepts; anything else is an error naming what was `expected`.
    fn parsed<T>(
        &mut self,
        node: &Node,
        expected: &'static str,
        parse: impl Fn(&str) -> Option<T>,
    ) -> Option<T> {
        let text = self.scalar(node)?;
        let parsed = parse(text);
        if parsed.is_none() {
            let value = text.to_owned();
            self.fail(Some(node.position), Problem::Invalid { value, expected });
        }
        parsed
    }

    /// A scalar that is not blank.
    fn text(&mut self, node: &Node, expected: &'static str) -> Option<String> {
        self.parsed(node, expected, |text| {
            (!text.trim().is_empty()).then(|| text.to_owned())
        })
    }

    /// A mapping's fields, each of which `allowed` must name; every other field is an error.
    fn fields<'a>(&mut self, node: &'a Node, allowed: &[&str]) -> Fields<'a> {
        let entries = self.entries(node);
        for entry in entries {
            if !allowed.contains(&entry.key.as_str()) {
                let unknown = Problem::UnknownField(entry.key.clone());
                self.fail(Some(entry.key_position), unknown);
            }
        }
        Fields(entries)
    }

    /// The field `name` of the mapping `node`; an error when it is not there.
    fn required<'a>(
        &mut self,
        node: &Node,
        fields: Fields<'a>,
        name: &'static str,
    ) -> Option<&'a Entry> {
        let entry = fields.get(name);
        if entry.is_none() {
            self.fail(Some(node.position), Problem::MissingField(name));
        }
        entry
    }
}

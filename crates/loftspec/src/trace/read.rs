//! The strict reading of the trace file: a field it does not define, a `format` other than
//! `loft-trace/1`, a link with both or neither of `tests` and `unverified`, an empty `tests`, a
//! blank reason and two links to one node are each an error at its line and column.

use std::collections::HashSet;

use super::{Evidence, FORMAT, Link, Test, Trace, TraceError};
use crate::canonical::Digest;
use crate::codec::Problem;
use crate::identity::Key;
use crate::yaml::{self, Entry, Location, Node, Position, Value};

/// Reads the trace file; no file holds no link.
///
/// # Errors
///
/// Every place where the file breaks its form, each at its line and column.
pub fn read(bytes: Option<&[u8]>) -> Result<Trace, Vec<TraceError>> {
    let Some(bytes) = bytes else {
        return Ok(Trace::default());
    };
    let root = yaml::read(bytes).map_err(|error| {
        vec![TraceError {
            location: Some(error.location),
            problem: Problem::Syntax(error.kind),
        }]
    })?;
    let mut reader = Reader::default();
    let trace = if let Some(root) = root {
        reader.file(&root)
    } else {
        reader.fail(None, Problem::MissingField("format"));
        Trace::default()
    };
    if reader.errors.is_empty() {
        Ok(trace)
    } else {
        Err(reader.errors)
    }
}

#[derive(Default)]
struct Reader {
    errors: Vec<TraceError>,
    nodes: HashSet<Key>,
}

impl Reader {
    fn fail(&mut self, position: Option<Position>, problem: Problem) {
        self.errors.push(TraceError {
            location: position.map(Location::At),
            problem,
        });
    }

    fn entries<'a>(&mut self, node: &'a Node) -> &'a [Entry] {
        if let Value::Mapping(entries) = &node.value {
            return entries;
        }
        self.fail(Some(node.position), Problem::Expected("a mapping"));
        &[]
    }

    /// A sequence's items; a field written with no value holds none.
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

    /// A scalar that `parse` accepts; anything else is an error naming what was `expected`.
    fn parsed<T>(
        &mut self,
        node: &Node,
        expected: &'static str,
        parse: impl Fn(&str) -> Option<T>,
    ) -> Option<T> {
        let Value::Scalar(scalar) = &node.value else {
            self.fail(Some(node.position), Problem::Expected("a scalar"));
            return None;
        };
        let parsed = parse(&scalar.text);
        if parsed.is_none() {
            let value = scalar.text.clone();
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

    /// Each field `allowed` names, by name; every other field is an error.
    fn fields<'a>(
        &mut self,
        node: &'a Node,
        allowed: &[&str],
    ) -> impl Fn(&str) -> Option<&'a Entry> + use<'a> {
        let entries = self.entries(node);
        for entry in entries {
            if !allowed.contains(&entry.key.as_str()) {
                let unknown = Problem::UnknownField(entry.key.clone());
                self.fail(Some(entry.key_position), unknown);
            }
        }
        move |name| entries.iter().find(|entry| entry.key == name)
    }

    /// The field `name` of `node`, which an error reports missing when it is not there.
    fn required<'a>(
        &mut self,
        node: &Node,
        entry: Option<&'a Entry>,
        name: &'static str,
    ) -> Option<&'a Entry> {
        if entry.is_none() {
            self.fail(Some(node.position), Problem::MissingField(name));
        }
        entry
    }

    fn file(&mut self, root: &Node) -> Trace {
        let field = self.fields(root, &["format", "links"]);
        if let Some(format) = self.required(root, field("format"), "format") {
            let expected = "`loft-trace/1`";
            self.parsed(&format.value, expected, |text| {
                (text == FORMAT).then_some(())
            });
        }
        let items = field("links").map_or(&[][..], |links| self.items(&links.value));
        let links = items.iter().filter_map(|item| self.link(item)).collect();
        Trace { links }
    }

    fn link(&mut self, item: &Node) -> Option<Link> {
        let field = self.fields(item, &["node", "reviewed", "tests", "unverified"]);
        let node = self
            .required(item, field("node"), "node")
            .and_then(|node| self.parsed(&node.value, "a key", |text| Key::parse(text).ok()));
        let reviewed = field("reviewed").map(|reviewed| {
            let expected = "64 lowercase hexadecimal digits";
            self.parsed(&reviewed.value, expected, |text| {
                text.parse::<Digest>().ok()
            })
        });
        let evidence = self.evidence(item, field("tests"), field("unverified"));
        if let Some(node) = node
            && !self.nodes.insert(node)
        {
            let twice = format!("a second link names the node {node}");
            self.fail(Some(item.position), Problem::Shape(twice));
        }
        Some(Link {
            node: node?,
            reviewed: reviewed.map_or(Some(None), |digest| digest.map(Some))?,
            evidence: evidence?,
            position: Some(item.position),
        })
    }

    /// `tests` or `unverified`: exactly one of them.
    fn evidence(
        &mut self,
        item: &Node,
        tests: Option<&Entry>,
        unverified: Option<&Entry>,
    ) -> Option<Evidence> {
        match (tests, unverified) {
            (Some(tests), None) => {
                let items = self.items(&tests.value);
                if items.is_empty() {
                    let empty = Problem::Shape("`tests` lists no test".to_owned());
                    self.fail(Some(tests.value.position), empty);
                }
                let read: Vec<Option<Test>> = items.iter().map(|test| self.test(test)).collect();
                let read: Option<Vec<Test>> = read.into_iter().collect();
                read.filter(|tests| !tests.is_empty()).map(Evidence::Tests)
            }
            (None, Some(reason)) => self
                .text(&reason.value, "a reason")
                .map(Evidence::Unverified),
            (both, _) => {
                let shape = if both.is_some() {
                    "a link holds both `tests` and `unverified`"
                } else {
                    "a link holds neither `tests` nor `unverified`"
                };
                self.fail(Some(item.position), Problem::Shape(shape.to_owned()));
                None
            }
        }
    }

    fn test(&mut self, item: &Node) -> Option<Test> {
        let field = self.fields(item, &["file", "name"]);
        let mut required = |name: &'static str, expected| {
            let entry = self.required(item, field(name), name)?;
            self.text(&entry.value, expected)
        };
        let file = required("file", "a path");
        let name = required("name", "a test's name");
        Some(Test {
            file: file?,
            name: name?,
        })
    }
}

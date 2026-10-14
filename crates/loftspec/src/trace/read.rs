//! The strict reading of the trace file: a field it does not define, a `format` other than
//! `loft-trace/1`, a link with both or neither of `tests` and `unverified`, an empty `tests`, a
//! blank reason and two links to one node are each an error at its line and column.

use std::collections::HashSet;

use super::{Evidence, FORMAT, Link, Test, Trace, TraceError};
use crate::canonical::Digest;
use crate::codec::{Problem, Strict};
use crate::identity::Key;
use crate::yaml::{self, Entry, Location, Node, Position};

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

impl Strict for Reader {
    fn fail(&mut self, position: Option<Position>, problem: Problem) {
        self.errors.push(TraceError {
            location: position.map(Location::At),
            problem,
        });
    }
}

impl Reader {
    fn file(&mut self, root: &Node) -> Trace {
        let fields = self.fields(root, &["format", "links"]);
        if let Some(format) = self.required(root, fields, "format") {
            let expected = "`loft-trace/1`";
            self.parsed(&format.value, expected, |text| {
                (text == FORMAT).then_some(())
            });
        }
        let items = fields
            .get("links")
            .map_or(&[][..], |links| self.items(&links.value));
        let links = items.iter().filter_map(|item| self.link(item)).collect();
        Trace { links }
    }

    fn link(&mut self, item: &Node) -> Option<Link> {
        let fields = self.fields(item, &["node", "reviewed", "tests", "unverified"]);
        let node = self
            .required(item, fields, "node")
            .and_then(|node| self.parsed(&node.value, "a key", |text| Key::parse(text).ok()));
        // A `reviewed` that does not parse leaves the link unread; none leaves it unreviewed.
        let reviewed = match fields.get("reviewed") {
            None => Some(None),
            Some(reviewed) => {
                let expected = "64 lowercase hexadecimal digits";
                let parse = |text: &str| text.parse::<Digest>().ok();
                self.parsed(&reviewed.value, expected, parse).map(Some)
            }
        };
        let evidence = self.evidence(item, fields.get("tests"), fields.get("unverified"));
        if let Some(node) = node
            && !self.nodes.insert(node)
        {
            let twice = format!("a second link names the node {node}");
            self.fail(Some(item.position), Problem::Shape(twice));
        }
        Some(Link {
            node: node?,
            reviewed: reviewed?,
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
                    return None;
                }
                // Every test is read, so each one's errors are reported.
                let read: Vec<Option<Test>> = items.iter().map(|test| self.test(test)).collect();
                read.into_iter().collect::<Option<_>>().map(Evidence::Tests)
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
        let fields = self.fields(item, &["file", "name"]);
        let mut required = |name: &'static str, expected| {
            let entry = self.required(item, fields, name)?;
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

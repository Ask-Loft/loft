//! The trace file in canonical layout: its links in the document order of their nodes, then the
//! links to keys no node carries, in the order of the keys.

use super::{Evidence, FORMAT, Link, Trace, order, places};
use crate::document::Document;
use crate::yaml::{self, Out};

/// The trace file's text for `trace`, its links ordered by the nodes of `document`.
#[must_use]
pub fn write(trace: &Trace, document: &Document) -> String {
    let places = places(document);
    let mut links: Vec<&Link> = trace.links.iter().collect();
    links.sort_by_key(|link| order(&places, link.node));
    yaml::write(&Out::Mapping(vec![
        ("format", Out::Token(FORMAT.to_owned())),
        (
            "links",
            Out::Sequence(links.into_iter().map(link).collect()),
        ),
    ]))
}

fn link(link: &Link) -> Out {
    let mut fields = vec![("node", Out::Token(link.node.to_string()))];
    if let Some(reviewed) = link.reviewed {
        fields.push(("reviewed", Out::Token(reviewed.to_string())));
    }
    fields.push(match &link.evidence {
        Evidence::Tests(tests) => {
            let tests = tests.iter().map(|test| {
                Out::Mapping(vec![
                    ("file", Out::Text(test.file.clone())),
                    ("name", Out::Text(test.name.clone())),
                ])
            });
            ("tests", Out::Sequence(tests.collect()))
        }
        Evidence::Unverified(reason) => ("unverified", Out::Text(reason.clone())),
    });
    Out::Mapping(fields)
}

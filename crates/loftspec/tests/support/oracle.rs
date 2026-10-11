//! A conforming YAML parser as the reader's oracle (ADR-14): the same document read by both
//! must give the same tree, scalar styles included.

use loftspec::yaml::{self, Style, Value};
use saphyr_parser::{Event, Parser, ScalarStyle};

/// A document as a plain tree both parsers can be compared on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tree {
    Mapping(Vec<(String, Tree)>),
    Sequence(Vec<Tree>),
    Scalar(Style, String),
    Null,
}

/// The tree the Format's reader gives.
pub fn ours(text: &str) -> Option<Tree> {
    yaml::read(text.as_bytes())
        .unwrap_or_else(|error| panic!("refused: {error}\n{text}"))
        .map(|node| from_node(&node))
}

fn from_node(node: &yaml::Node) -> Tree {
    match &node.value {
        Value::Mapping(entries) => Tree::Mapping(
            entries
                .iter()
                .map(|entry| (entry.key.clone(), from_node(&entry.value)))
                .collect(),
        ),
        Value::Sequence(items) => Tree::Sequence(items.iter().map(from_node).collect()),
        Value::Scalar(scalar) => Tree::Scalar(scalar.style, scalar.text.clone()),
        Value::Empty => Tree::Null,
    }
}

/// The tree saphyr-parser gives.
pub fn theirs(text: &str) -> Option<Tree> {
    let mut events = Parser::new_from_str(text).map(|event| {
        event
            .unwrap_or_else(|error| panic!("saphyr refused: {error}\n{text}"))
            .0
    });
    let mut stack: Vec<(Tree, Option<String>)> = Vec::new();
    let mut root = None;
    for event in events.by_ref() {
        let node = match event {
            Event::MappingStart(..) => {
                stack.push((Tree::Mapping(Vec::new()), None));
                continue;
            }
            Event::SequenceStart(..) => {
                stack.push((Tree::Sequence(Vec::new()), None));
                continue;
            }
            Event::MappingEnd | Event::SequenceEnd => stack.pop().expect("an open collection").0,
            Event::Scalar(text, style, ..) => scalar(&text, style),
            _ => continue,
        };
        match stack.last_mut() {
            None => root = Some(node),
            Some((Tree::Sequence(items), _)) => items.push(node),
            Some((Tree::Mapping(entries), pending)) => match pending.take() {
                Some(key) => entries.push((key, node)),
                None => match node {
                    Tree::Scalar(_, key) => *pending = Some(key),
                    other => panic!("a non-scalar key: {other:?}"),
                },
            },
            Some(_) => unreachable!("only collections are pushed"),
        }
    }
    root
}

fn scalar(text: &str, style: ScalarStyle) -> Tree {
    match style {
        ScalarStyle::Plain if text.is_empty() => Tree::Null,
        ScalarStyle::Plain => Tree::Scalar(Style::Plain, text.to_owned()),
        ScalarStyle::DoubleQuoted => Tree::Scalar(Style::DoubleQuoted, text.to_owned()),
        ScalarStyle::Literal => Tree::Scalar(Style::Literal, text.to_owned()),
        other => panic!("a style the subset does not admit: {other:?}"),
    }
}

/// Asserts that both parsers read `text` to the same tree.
pub fn agree(text: &str) {
    assert_eq!(ours(text), theirs(text), "{text}");
}

//! A Rust test: a function, at any depth of modules, implementations and functions, with the
//! attributes written directly before it; and, inside a macro invocation such as `proptest!`,
//! the tokens from `fn` and its name through the next group in braces, with the attributes
//! written directly before them.

use tree_sitter::Node;

use super::{Candidate, each, text_of};

pub(super) fn tests(root: Node<'_>, text: &str) -> Vec<Candidate> {
    let mut found = Vec::new();
    each(root, &mut |node| match node.kind() {
        "function_item" => found.extend(function(node, text)),
        "token_tree" => found.extend(in_macro(node, text)),
        _ => {}
    });
    found
}

/// The function `node`, from its first attribute.
fn function(node: Node<'_>, text: &str) -> Option<Candidate> {
    let name = text_of(node.child_by_field_name("name")?, text).to_owned();
    let mut first = node;
    let mut before = node;
    while let Some(previous) = before.prev_sibling() {
        match previous.kind() {
            "attribute_item" => first = previous,
            kind if kind.ends_with("comment") => {}
            _ => break,
        }
        before = previous;
    }
    Some(Candidate::between(name, first, node))
}

/// The functions written as tokens directly in the group `node` of a macro invocation.
fn in_macro(node: Node<'_>, text: &str) -> Vec<Candidate> {
    let mut cursor = node.walk();
    let tokens: Vec<Node<'_>> = node.children(&mut cursor).collect();
    let mut found = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        if text_of(*token, text) != "fn" {
            continue;
        }
        let Some(name) = tokens
            .get(at + 1)
            .filter(|name| name.kind() == "identifier")
        else {
            continue;
        };
        let body = tokens[at + 2..]
            .iter()
            .find(|token| token.kind() == "token_tree" && text_of(**token, text).starts_with('{'));
        let Some(body) = body else {
            continue;
        };
        let first = attributes(&tokens[..at], text).unwrap_or(*token);
        found.push(Candidate::between(
            text_of(*name, text).to_owned(),
            first,
            *body,
        ));
    }
    found
}

/// The first token of the attributes, each `#` and a group in brackets, that end `tokens`.
fn attributes<'tree>(tokens: &[Node<'tree>], text: &str) -> Option<Node<'tree>> {
    let mut first = None;
    let mut end = tokens.len();
    while end >= 2 {
        let (hash, group) = (tokens[end - 2], tokens[end - 1]);
        if text_of(hash, text) != "#" || !text_of(group, text).starts_with('[') {
            break;
        }
        first = Some(hash);
        end -= 2;
    }
    first
}

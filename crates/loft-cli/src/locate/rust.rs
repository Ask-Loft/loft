//! A Rust test: a function, at any depth of modules, implementations and functions, with the
//! attributes written directly before it; and, inside a macro invocation such as `proptest!`,
//! the tokens from `fn` and its name through the next group in braces, with the qualifiers such as
//! `pub` and `async` and the attributes written directly before them, whatever comments lie among
//! them. A `fn` followed by `;` before any group in braces declares no test.

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

/// The functions written as tokens directly in the group `node` of a macro invocation; its
/// comments are passed over.
fn in_macro(node: Node<'_>, text: &str) -> Vec<Candidate> {
    let mut cursor = node.walk();
    let tokens: Vec<Node<'_>> = node
        .children(&mut cursor)
        .filter(|token| !token.kind().ends_with("comment"))
        .collect();
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
        let Some(body) = body(&tokens[at + 2..], text) else {
            continue;
        };
        let qualified = qualifiers(&tokens[..at], text);
        let first = attributes(&tokens[..qualified], text)
            .or_else(|| tokens.get(qualified).copied())
            .unwrap_or(*token);
        found.push(Candidate::between(
            text_of(*name, text).to_owned(),
            first,
            body,
        ));
    }
    found
}

/// The group in braces that ends the signature in `tokens`; none where a `;` comes first.
fn body<'tree>(tokens: &[Node<'tree>], text: &str) -> Option<Node<'tree>> {
    tokens
        .iter()
        .find(|token| {
            let written = text_of(**token, text);
            written == ";" || (token.kind() == "token_tree" && written.starts_with('{'))
        })
        .copied()
        .filter(|token| text_of(*token, text) != ";")
}

/// The words that may come between a function's attributes and its `fn`.
const QUALIFIERS: [&str; 5] = ["pub", "async", "unsafe", "const", "extern"];

/// The index in `tokens` where the qualifiers that end it begin: `pub` with an optional group in
/// parentheses, `async`, `unsafe`, `const`, and `extern` with an optional ABI string.
fn qualifiers(tokens: &[Node<'_>], text: &str) -> usize {
    let mut start = tokens.len();
    while start > 0 {
        let token = tokens[start - 1];
        let written = text_of(token, text);
        let before = start.checked_sub(2).map(|at| text_of(tokens[at], text));
        let step = if QUALIFIERS.contains(&written) {
            1
        } else if (token.kind() == "token_tree"
            && written.starts_with('(')
            && before == Some("pub"))
            || (token.kind() == "string_literal" && before == Some("extern"))
        {
            2
        } else {
            break;
        };
        start -= step;
    }
    start
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

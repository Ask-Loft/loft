//! A Python test: a function or a class, at any depth, with its decorators.

use tree_sitter::Node;

use super::{Candidate, each, text_of};

pub(super) fn tests(root: Node<'_>, text: &str) -> Vec<Candidate> {
    let mut found = Vec::new();
    each(root, &mut |node| {
        if matches!(node.kind(), "function_definition" | "class_definition")
            && let Some(name) = node.child_by_field_name("name")
        {
            let decorated = node
                .parent()
                .filter(|parent| parent.kind() == "decorated_definition");
            let whole = decorated.unwrap_or(node);
            found.push(Candidate::spanning(text_of(name, text).to_owned(), whole));
        }
    });
    found
}

//! A Java test: a method or a class, at any depth of classes, interfaces, enums and records, with
//! the annotations and modifiers written before it, which the grammar holds inside the declaration.

use tree_sitter::Node;

use super::{Candidate, each, text_of};

pub(super) fn tests(root: Node<'_>, text: &str) -> Vec<Candidate> {
    let mut found = Vec::new();
    each(root, &mut |node| {
        if matches!(node.kind(), "method_declaration" | "class_declaration")
            && let Some(name) = node.child_by_field_name("name")
        {
            found.push(Candidate::spanning(text_of(name, text).to_owned(), node));
        }
    });
    found
}

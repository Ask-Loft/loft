//! A Go test: a function or a method, and a subtest, a call to a method `Run` whose first
//! argument is a string literal that names it.

use tree_sitter::Node;

use super::{Candidate, each, strings, text_of};

pub(super) fn tests(root: Node<'_>, text: &str) -> Vec<Candidate> {
    let mut found = Vec::new();
    each(root, &mut |node| {
        let name = match node.kind() {
            "function_declaration" | "method_declaration" => node
                .child_by_field_name("name")
                .map(|name| text_of(name, text).to_owned()),
            "call_expression" => subtest(node, text),
            _ => None,
        };
        if let Some(name) = name {
            found.push(Candidate::spanning(name, node));
        }
    });
    found
}

/// The name of the subtest that the call `node` runs.
fn subtest(node: Node<'_>, text: &str) -> Option<String> {
    let function = node.child_by_field_name("function")?;
    let field = function.child_by_field_name("field")?;
    if function.kind() != "selector_expression" || text_of(field, text) != "Run" {
        return None;
    }
    let arguments = node.child_by_field_name("arguments")?;
    strings::value(arguments.named_child(0)?, text)
}

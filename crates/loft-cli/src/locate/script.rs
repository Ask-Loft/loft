//! A JavaScript or TypeScript test: a call to `it`, `test`, `describe`, `context`, `specify` or
//! `bench`, directly or through a modifier such as `.only` or a call such as `.each(…)`, whose
//! first argument is a string, or a template literal without a substitution, that names it. Of
//! calls nested in one another that carry one name, the outermost is the test.

use tree_sitter::Node;

use super::{Candidate, each, strings, text_of};

/// The functions that register a test or a group of tests.
const REGISTERS: [&str; 6] = ["it", "test", "describe", "context", "specify", "bench"];

/// The properties through which a registering function still registers.
const MODIFIERS: [&str; 6] = ["only", "skip", "todo", "concurrent", "sequential", "fails"];

/// The properties that are called first and return a registering function.
const CALLED: [&str; 3] = ["each", "skipIf", "runIf"];

pub(super) fn tests(root: Node<'_>, text: &str) -> Vec<Candidate> {
    let mut found = Vec::new();
    each(root, &mut |node| {
        if node.kind() == "call_expression"
            && let Some(name) = registered(node, text)
        {
            found.push(Candidate::spanning(name, node));
        }
    });
    outermost(found)
}

/// The name of the test that the call `node` registers.
fn registered(node: Node<'_>, text: &str) -> Option<String> {
    if !registers(node.child_by_field_name("function")?, text) {
        return None;
    }
    let arguments = node.child_by_field_name("arguments")?;
    strings::value(arguments.named_child(0)?, text)
}

/// Whether calling `callee` registers a test.
fn registers(callee: Node<'_>, text: &str) -> bool {
    let property = |node: Node<'_>| {
        node.child_by_field_name("property")
            .map(|property| text_of(property, text))
    };
    match callee.kind() {
        "identifier" => REGISTERS.contains(&text_of(callee, text)),
        "member_expression" => {
            property(callee).is_some_and(|name| MODIFIERS.contains(&name))
                && callee
                    .child_by_field_name("object")
                    .is_some_and(|object| registers(object, text))
        }
        "call_expression" => callee.child_by_field_name("function").is_some_and(|inner| {
            inner.kind() == "member_expression"
                && property(inner).is_some_and(|name| CALLED.contains(&name))
                && inner
                    .child_by_field_name("object")
                    .is_some_and(|object| registers(object, text))
        }),
        _ => false,
    }
}

/// `found` without each test that lies within another test of the same name.
fn outermost(found: Vec<Candidate>) -> Vec<Candidate> {
    let within = |inner: &Candidate, outer: &Candidate| {
        inner.name == outer.name
            && outer.start <= inner.start
            && inner.end <= outer.end
            && (outer.start, outer.end) != (inner.start, inner.end)
    };
    let kept: Vec<bool> = found
        .iter()
        .map(|candidate| !found.iter().any(|other| within(candidate, other)))
        .collect();
    found
        .into_iter()
        .zip(kept)
        .filter_map(|(candidate, kept)| kept.then_some(candidate))
        .collect()
}

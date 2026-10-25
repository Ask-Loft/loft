//! The words of a node and of a decision, as a `spec-change` view's diff shows them before and
//! after the change (the See a spec change Feature): a Requirement's statement, a criterion's
//! name, a decision's title and outcome, and the name and body of any other node.

use loftspec::decision::Decision;
use loftspec::node::Node;
use serde_json::{Value, json};

/// The words of `node`.
pub fn of_node(node: Node<'_>) -> String {
    let body = match node {
        Node::Requirement(_) | Node::Criterion(_) => None,
        Node::Section(section) => Some(section.body.as_str()),
        _ => node.details().and_then(|details| details.body.as_deref()),
    };
    joined(&node.name(), body)
}

/// The words of `decision`.
pub fn of_decision(decision: &Decision) -> String {
    joined(&decision.title, decision.state.outcome())
}

/// The words a node or a decision had in the base, `before`, and has in the tree, `after`; a node
/// added has none before, and a node removed none after.
pub fn pair(before: Option<&str>, after: Option<&str>) -> Value {
    let mut words = json!({});
    if let Some(before) = before {
        words["before"] = json!(before);
    }
    if let Some(after) = after {
        words["after"] = json!(after);
    }
    words
}

/// A name and the body beneath it, a blank line between them.
fn joined(name: &str, body: Option<&str>) -> String {
    match body.map(str::trim).filter(|body| !body.is_empty()) {
        Some(body) => format!("{name}\n\n{body}"),
        None => name.to_owned(),
    }
}

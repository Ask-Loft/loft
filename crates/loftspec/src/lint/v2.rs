//! The rules the v2 text form adds (F-21): a heading inside a body (`V2-001`), a statement stored
//! as text without slots (`V2-002`), and a label reference in prose without its `mentions` edge
//! (`V2-003`).

use std::collections::BTreeSet;

use super::context::Context;
use super::{Raw, RuleId};
use crate::document::Edge;
use crate::ids::ElementId;
use crate::node::Node;
use crate::prose::{heading_lines, label_references};
use crate::statement::Statement;

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    let mentions: BTreeSet<(ElementId, ElementId)> = context
        .document
        .edges
        .iter()
        .filter_map(|edge| match *edge {
            Edge::Mentions { source, target } => Some((source, target)),
            _ => None,
        })
        .collect();
    for item in &context.items {
        let key = item.id.key();
        let body = item.body().unwrap_or_default();
        for line in heading_lines(body) {
            out.push(
                Raw::new(
                    RuleId::V2001,
                    vec![key],
                    "The body holds a heading; use a Section.",
                )
                .line(Some(line)),
            );
        }
        if let Node::Requirement(requirement) = item.node
            && matches!(requirement.statement, Statement::Text(_))
        {
            out.push(Raw::new(
                RuleId::V2002,
                vec![key],
                "The statement is stored as text, without slots.",
            ));
        }
        let name = item.name();
        let counters = &context.document.counters;
        let references = label_references(&name)
            .into_iter()
            .map(|label| (label, None))
            .chain(body_references(body));
        let source = ElementId::from(item.id);
        for (label, line) in references {
            let message = match context.document.bindings.resolve(label) {
                Some(target) if mentions.contains(&(source, target)) => continue,
                Some(_) => "The reference has no `mentions` edge.",
                // A label never allocated is not a reference, such as the curve name `P-256`.
                None if !counters.has_allocated(label) => continue,
                None => "The reference names no element.",
            };
            out.push(
                Raw::new(RuleId::V2003, vec![key], message)
                    .term(label.to_string())
                    .line(line),
            );
        }
    }
}

/// The label references of a body, each with its body line.
fn body_references(body: &str) -> impl Iterator<Item = (crate::identity::Label, Option<usize>)> {
    body.lines().enumerate().flat_map(|(index, line)| {
        label_references(line)
            .into_iter()
            .map(move |label| (label, Some(index + 1)))
    })
}

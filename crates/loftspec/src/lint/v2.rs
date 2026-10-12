//! The rules the v2 text form adds (F-21): a heading inside a body (`V2-001`), a statement stored
//! as text without slots (`V2-002`), a label reference in prose without its `mentions` edge
//! (`V2-003`), and a citation of a withdrawn label that a publish cannot rewrite (`V2-004`).

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
        let mentioned = |target: ElementId| mentions.contains(&(source, target));
        for (label, line) in references {
            let holder = context.document.bindings.resolve(label);
            let withdrawn = withdrawn_from(context, label).any(mentioned);
            let (rule, message) = match holder {
                Some(holder) if mentioned(holder) && withdrawn => (
                    RuleId::V2004,
                    "The label is withdrawn from one mentioned element and held by another; \
                     the next publish cannot rewrite it.",
                ),
                Some(holder) if mentioned(holder) => continue,
                _ if withdrawn => (
                    RuleId::V2003,
                    "The label is withdrawn from a mentioned element; the next publish rewrites it.",
                ),
                Some(_) => (RuleId::V2003, "The reference has no `mentions` edge."),
                // A label never allocated is not a reference, such as the curve name `P-256`.
                None if !counters.has_allocated(label) => continue,
                None => (RuleId::V2003, "The reference names no element."),
            };
            out.push(
                Raw::new(rule, vec![key], message)
                    .term(label.to_string())
                    .line(line),
            );
        }
    }
}

/// The elements `label` was withdrawn from.
fn withdrawn_from<'a>(
    context: &'a Context<'_>,
    label: crate::identity::Label,
) -> impl Iterator<Item = ElementId> + 'a {
    context
        .document
        .bindings
        .iter()
        .filter(move |binding| binding.withdrawn.contains(&label))
        .map(|binding| binding.element)
}

/// The label references of a body, each with its body line.
fn body_references(body: &str) -> impl Iterator<Item = (crate::identity::Label, Option<usize>)> {
    body.lines().enumerate().flat_map(|(index, line)| {
        label_references(line)
            .into_iter()
            .map(move |label| (label, Some(index + 1)))
    })
}

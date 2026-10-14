//! The rules the v2 text form adds (F-21): a heading inside a body (`V2-001`), a statement stored
//! as text without slots (`V2-002`), a label reference in prose without its `mentions` edge
//! (`V2-003`), a citation of a withdrawn label that a publish cannot rewrite (`V2-004`), and a
//! Markdown construct in a body outside the Format's subset (`V2-009`).
//! What a publish does with a citation is the bindings' to say, so lint and the publish agree.

use std::collections::BTreeSet;

use super::context::Context;
use super::markdown::outside_subset;
use super::{Raw, RuleId};
use crate::bindings::Citation;
use crate::document::Edge;
use crate::identity::Key;
use crate::ids::ElementId;
use crate::node::Node;
use crate::prose::{LabelSpan, heading_lines, label_spans};
use crate::statement::Statement;

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    let mentions: BTreeSet<(ElementId, Key)> = context
        .document
        .edges
        .iter()
        .filter_map(|edge| match *edge {
            Edge::Mentions { source, target } => Some((source, target.key())),
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
        for (line, construct) in outside_subset(body) {
            out.push(
                Raw::new(
                    RuleId::V2009,
                    vec![key],
                    format!("The body holds {construct}, outside the Markdown subset."),
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
        let references = label_spans(&name)
            .into_iter()
            .map(|span| (span, None))
            .chain(body_references(body));
        let source = ElementId::from(item.id);
        let mentioned = |target: Key| mentions.contains(&(source, target));
        for (span, line) in references {
            let Some((rule, message)) = finding(context, &span, mentioned) else {
                continue;
            };
            out.push(
                Raw::new(rule, vec![key], message)
                    .term(span.label.to_string())
                    .line(line),
            );
        }
    }
}

/// The finding a label reference yields in an element that mentions the keys `mentioned`
/// accepts: by what the next publish does with it, then by whether it names an element.
fn finding(
    context: &Context<'_>,
    span: &LabelSpan,
    mentioned: impl Fn(Key) -> bool + Copy,
) -> Option<(RuleId, &'static str)> {
    let bindings = &context.document.bindings;
    let citation = if span.qualified {
        Citation::NotWithdrawn
    } else {
        bindings.citation(span.label, mentioned)
    };
    Some(match citation {
        Citation::Rewritten(_) => (
            RuleId::V2003,
            "The label is withdrawn from a mentioned element; the next publish rewrites it.",
        ),
        Citation::HolderMentioned => (
            RuleId::V2004,
            "The label is withdrawn from one mentioned element and held by another; the next \
             publish cannot rewrite it.",
        ),
        Citation::Ambiguous => (
            RuleId::V2004,
            "The label is withdrawn from two mentioned elements; the next publish cannot rewrite \
             it.",
        ),
        Citation::NotWithdrawn => match bindings.resolve(span.label) {
            Some(holder) if mentioned(holder.key()) => return None,
            Some(_) => (RuleId::V2003, "The reference has no `mentions` edge."),
            // A label never allocated is not a reference, such as the curve name `P-256`.
            None if !context.document.counters.has_allocated(span.label) => return None,
            None => (RuleId::V2003, "The reference names no element."),
        },
    })
}

/// The label references of a body, each with its body line.
fn body_references(body: &str) -> impl Iterator<Item = (LabelSpan, Option<usize>)> {
    body.lines().enumerate().flat_map(|(index, line)| {
        label_spans(line)
            .into_iter()
            .map(move |span| (span, Some(index + 1)))
    })
}

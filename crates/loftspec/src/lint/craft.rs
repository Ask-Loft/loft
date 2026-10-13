//! The rules of the writing craft (F-21): a criterion whose name states more than one outcome
//! (`V2-005`), a Requirement whose size suggests a Feature (`V2-006`), and a proper name after
//! an article in a statement's system (`V2-008`). A normative word in a body (`V2-007`) is a
//! lexical rule, read with the language rules' scan in `language.rs`.

use super::context::{Context, Item};
use super::text::{Body, without_code_spans};
use super::{LintConfig, Raw, RuleId};
use crate::node::Node;
use crate::statement::Statement;

pub(super) fn check(context: &Context<'_>, config: &LintConfig, out: &mut Vec<Raw>) {
    for item in &context.items {
        match item.node {
            Node::Criterion(criterion) if is_compound(&criterion.name) => {
                out.push(Raw::new(
                    RuleId::V2005,
                    vec![item.id.key()],
                    "The criterion states more than one outcome.",
                ));
            }
            Node::Requirement(requirement) => {
                let words = item
                    .body()
                    .map_or(0, |body| Body::read(body).tokens().0.len());
                oversized(item, requirement.criteria.len(), words, config, out);
                proper_name(item, &requirement.statement, config, out);
            }
            _ => {}
        }
    }
}

/// Whether a criterion's name, outside its code spans, holds a semicolon or more than one
/// sentence. A sentence ends at a full stop, question mark or exclamation mark followed by a
/// space and a capital letter.
fn is_compound(name: &str) -> bool {
    let shown = without_code_spans(name);
    if shown.contains(';') {
        return true;
    }
    let chars: Vec<char> = shown.chars().collect();
    chars.windows(3).any(|window| {
        matches!(window[0], '.' | '?' | '!') && window[1] == ' ' && window[2].is_uppercase()
    })
}

/// `V2-006`: one finding for each limit a Requirement passes, its criteria and its body's words.
fn oversized(
    item: &Item<'_>,
    criteria: usize,
    words: usize,
    config: &LintConfig,
    out: &mut Vec<Raw>,
) {
    let key = item.id.key();
    if criteria > config.max_criteria() {
        let message = format!(
            "The Requirement holds {criteria} criteria, more than {}; it may be a Feature.",
            config.max_criteria()
        );
        out.push(Raw::new(RuleId::V2006, vec![key], message).term("criteria"));
    }
    if words > config.max_body_words() {
        let message = format!(
            "The Requirement's body holds {words} words, more than {}; it may be a Feature.",
            config.max_body_words()
        );
        out.push(Raw::new(RuleId::V2006, vec![key], message).term("body"));
    }
}

/// `V2-008`: a formed statement whose system is "the" followed by a listed proper name.
fn proper_name(item: &Item<'_>, statement: &Statement, config: &LintConfig, out: &mut Vec<Raw>) {
    let Statement::Form(form) = statement else {
        return;
    };
    let system = form.parts().1.trim();
    let Some(named) = system
        .strip_prefix("the ")
        .or_else(|| system.strip_prefix("The "))
    else {
        return;
    };
    if let Some(name) = config.proper_names().iter().find(|name| *name == named) {
        out.push(
            Raw::new(
                RuleId::V2008,
                vec![item.id.key()],
                "A proper name stands without an article.",
            )
            .term(name),
        );
    }
}

//! The rules over the term list (F-19, F-20): `SH-011`, a name or alias that two entries carry,
//! and `LQ-010`, an alias of a term in the prose of a node. Names, aliases and prose are compared
//! as words in lower case, the last word with or without a final plural.

use super::context::{Context, Item};
use super::text::{Body, tokens, without_code_spans, words};
use super::{Raw, RuleId};
use crate::document::TermEntry;

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    let terms = &context.document.terms;
    if terms.is_empty() {
        return;
    }
    shared_names(terms, out);
    let aliases: Vec<(&TermEntry, &str, Vec<String>)> = terms
        .iter()
        .flat_map(|term| term.aliases.iter().map(move |alias| (term, alias.as_str())))
        .map(|(term, alias)| (term, alias, phrase(alias)))
        .filter(|(.., words)| !words.is_empty())
        .collect();
    for item in &context.items {
        aliases_in(item, &aliases, out);
    }
}

/// A name or alias as compared: its words in lower case, the last one singular.
pub(crate) fn phrase(text: &str) -> Vec<String> {
    let mut words = tokens(text);
    if let Some(last) = words.last_mut() {
        *last = singular(last);
    }
    words
}

/// A word without a final plural: `-ies` as `-y`, `-sses`, `-ches`, `-shes` and `-xes` without
/// `-es`, and `-s` without it unless the word ends in `-ss`, `-us` or `-is`.
pub(crate) fn singular(word: &str) -> String {
    let long = |n: usize| word.chars().count() > n;
    if long(4) && word.ends_with("ies") {
        return format!("{}y", &word[..word.len() - 3]);
    }
    if long(4)
        && ["sses", "ches", "shes", "xes"]
            .iter()
            .any(|end| word.ends_with(end))
    {
        return word[..word.len() - 2].to_owned();
    }
    if long(3) && word.ends_with('s') && !["ss", "us", "is"].iter().any(|end| word.ends_with(end)) {
        return word[..word.len() - 1].to_owned();
    }
    word.to_owned()
}

/// `SH-011`: each pair of entries that carry one name or alias, and each alias equal to its own
/// term's name.
fn shared_names(terms: &[TermEntry], out: &mut Vec<Raw>) {
    let mut entries: Vec<&TermEntry> = terms.iter().collect();
    entries.sort_by_key(|term| (term.name.as_str(), term.id));
    let names = |term: &TermEntry| -> Vec<Vec<String>> {
        let mut names: Vec<Vec<String>> = std::iter::once(&term.name)
            .chain(&term.aliases)
            .map(|name| phrase(name))
            .collect();
        names.sort();
        names.dedup();
        names
    };
    for term in &entries {
        let own = phrase(&term.name);
        for alias in &term.aliases {
            if phrase(alias) == own {
                let message = "The alias is the term's own name.";
                out.push(Raw::new(RuleId::Sh011, vec![term.id.key()], message).term(alias));
            }
        }
    }
    for (at, first) in entries.iter().enumerate() {
        let carried = names(first);
        for second in &entries[at + 1..] {
            let shared = names(second)
                .into_iter()
                .find(|name| carried.contains(name));
            if let Some(shared) = shared {
                let message = format!(
                    "`{}` and `{}` both carry the name.",
                    first.name, second.name
                );
                let keys = vec![first.id.key(), second.id.key()];
                out.push(Raw::new(RuleId::Sh011, keys, message).term(shared.join(" ")));
            }
        }
    }
}

/// `LQ-010`: each alias in the name, statement or body of a node, once per node and alias.
fn aliases_in(item: &Item<'_>, aliases: &[(&TermEntry, &str, Vec<String>)], out: &mut Vec<Raw>) {
    let (name, _) = words(&without_code_spans(&item.name()));
    let body = item.body().map(Body::read);
    let (body_tokens, lines, _) = body.as_ref().map(Body::words).unwrap_or_default();
    for (term, alias, phrase) in aliases {
        let line = if occurs(&name, phrase).is_some() {
            None
        } else if let Some(at) = occurs(&body_tokens, phrase) {
            Some(lines[at])
        } else {
            continue;
        };
        let message = format!("The text uses an alias of the term `{}`.", term.name);
        out.push(
            Raw::new(RuleId::Lq010, vec![item.id.key()], message)
                .term(*alias)
                .line(line),
        );
    }
}

/// Whether the name or alias `phrase` occurs in `name` or `body`, compared as `LQ-010` compares
/// it.
pub(crate) fn occurs_in(name: &str, body: Option<&str>, phrase_text: &str) -> bool {
    let phrase = phrase(phrase_text);
    if phrase.is_empty() {
        return false;
    }
    let (name, _) = words(&without_code_spans(name));
    occurs(&name, &phrase).is_some()
        || body.is_some_and(|body| occurs(&Body::read(body).words().0, &phrase).is_some())
}

/// Where `phrase` first occurs in `tokens`, its last word with or without a final plural.
fn occurs(tokens: &[String], phrase: &[String]) -> Option<usize> {
    let (last, lead) = phrase.split_last()?;
    tokens.windows(phrase.len()).position(|window| {
        let (word, before) = window
            .split_last()
            .expect("a window of the phrase's length");
        before == lead && singular(word) == *last
    })
}

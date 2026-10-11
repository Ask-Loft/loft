//! Recognising the five basic EARS forms over a sentence's tokens: the same classification as
//! the language rule that reports a statement outside the five forms, so a sentence the Format
//! stores as slots is one that rule accepts. Spans index [`crate::text::tokenize`].

use crate::text::{token_ranges, tokenize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    Ubiquitous,
    EventDriven,
    StateDriven,
    Unwanted,
    Optional,
}

/// A half-open `[start, end)` range of token indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// A sentence split into its precondition, system and response spans. The ubiquitous form has
/// no precondition. The system span leaves out a leading `the`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Parse {
    pub(crate) form: Form,
    pub(crate) precondition: Option<Span>,
    pub(crate) response: Span,
}

/// Parses `text` as one anchored basic EARS form; combined forms and empty placeholders give
/// `None`. The system phrase may be a common noun with `the` or a bare proper name: it starts
/// after the comma that closes the precondition, or failing that at the precondition's `the`.
pub(crate) fn parse(text: &str) -> Option<Parse> {
    let tokens = tokenize(text);
    let ranges = token_ranges(text);
    let comma_before =
        |index: usize| index > 0 && text[ranges[index - 1].end..ranges[index].start].contains(',');
    ubiquitous(&tokens, &comma_before)
        .or_else(|| prefixed(&tokens, &comma_before, "when", Form::EventDriven))
        .or_else(|| prefixed(&tokens, &comma_before, "while", Form::StateDriven))
        .or_else(|| unwanted(&tokens))
        .or_else(|| prefixed(&tokens, &comma_before, "where", Form::Optional))
}

/// A ubiquitous system phrase is a whole noun phrase: it opens with no prefix word or `then`,
/// holds no comma, and a bare proper name holds no later `the` that would mark a lead-in clause.
fn ubiquitous(tokens: &[String], comma_before: &dyn Fn(usize) -> bool) -> Option<Parse> {
    let first = tokens.first()?;
    if is_prefix(first) || first == "then" {
        return None;
    }
    let shall = shall_after(tokens, 1)?;
    if (1..=shall).any(comma_before)
        || (first != "the" && tokens[1..shall].iter().any(|token| token == "the"))
    {
        return None;
    }
    system_is_nonempty(tokens, 0, shall)?;
    Some(Parse {
        form: Form::Ubiquitous,
        precondition: None,
        response: span(shall + 1, tokens.len()),
    })
}

fn prefixed(
    tokens: &[String],
    comma_before: &dyn Fn(usize) -> bool,
    prefix: &str,
    form: Form,
) -> Option<Parse> {
    if tokens.first().is_none_or(|token| token != prefix) {
        return None;
    }
    let shall = shall_after(tokens, 2)?;
    let comma = (2..shall).rev().find(|&index| comma_before(index));
    let boundary = comma.or_else(|| (2..shall).find(|&index| tokens[index] == "the"))?;
    if contains_other_prefix(&tokens[1..boundary], prefix) {
        return None;
    }
    system_is_nonempty(tokens, boundary, shall)?;
    Some(Parse {
        form,
        precondition: Some(span(1, boundary)),
        response: span(shall + 1, tokens.len()),
    })
}

fn unwanted(tokens: &[String]) -> Option<Parse> {
    if tokens.first().is_none_or(|token| token != "if") {
        return None;
    }
    (2..tokens.len().saturating_sub(1)).find_map(|then| {
        if tokens[then] != "then" || contains_other_prefix(&tokens[1..then], "if") {
            return None;
        }
        let shall = shall_after(tokens, then + 2)?;
        system_is_nonempty(tokens, then + 1, shall)?;
        Some(Parse {
            form: Form::Unwanted,
            precondition: Some(span(1, then)),
            response: span(shall + 1, tokens.len()),
        })
    })
}

/// The system phrase in `[start, shall)`, without a leading `the`, is not empty.
fn system_is_nonempty(tokens: &[String], start: usize, shall: usize) -> Option<()> {
    let start = if tokens.get(start).is_some_and(|token| token == "the") {
        start + 1
    } else {
        start
    };
    (start < shall).then_some(())
}

fn is_prefix(token: &str) -> bool {
    matches!(token, "when" | "while" | "if" | "where")
}

const fn span(start: usize, end: usize) -> Span {
    Span { start, end }
}

fn shall_after(tokens: &[String], first_position: usize) -> Option<usize> {
    (first_position..tokens.len().saturating_sub(1)).find(|&index| tokens[index] == "shall")
}

fn contains_other_prefix(tokens: &[String], allowed: &str) -> bool {
    tokens
        .iter()
        .any(|token| is_prefix(token) && token != allowed)
}

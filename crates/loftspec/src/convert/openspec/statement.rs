//! A requirement's text split into the Requirement's statement and body, with the modal word
//! `SHALL` or `MUST` written as `shall` when that gives the statement slots.

use std::ops::Range;

use super::read::{Source, verbatim};
use crate::statement::Statement;

/// The statement and body a requirement's text gives.
#[derive(Debug)]
pub(super) struct Split {
    pub statement: Statement,
    pub body: Option<String>,
    /// The modal word, `SHALL` or `MUST`, written as `shall` to give the statement slots.
    pub rewritten: Option<&'static str>,
    /// Each metadata line's label and value, in source order.
    pub metadata: Vec<(String, String)>,
    /// Whether the text, or the name when there is no text, holds `SHALL` or `MUST`.
    pub modal: bool,
}

/// Splits the text of a requirement named `name`.
pub(super) fn split(source: &Source, text: Range<usize>, name: &str) -> Split {
    let mut metadata = Vec::new();
    let mut rest = Vec::new();
    let mut modal = false;
    for (line, fenced) in source.lines(text) {
        if let Some((label, value)) = (!fenced).then(|| metadata_entry(line)).flatten() {
            metadata.push((label.to_owned(), value.to_owned()));
            continue;
        }
        modal |= !fenced && first_modal(line).is_some();
        rest.push(line);
    }
    let first = rest.iter().position(|line| !line.trim().is_empty());
    let (sentence, after) = first.map_or((None, &rest[..0]), |first| {
        let length = rest[first..]
            .iter()
            .position(|line| line.trim().is_empty())
            .unwrap_or(rest.len() - first);
        let sentence = rest[first..first + length]
            .iter()
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join(" ");
        (Some(sentence), &rest[first + length..])
    });
    let sentence = sentence.unwrap_or_else(|| {
        modal = first_modal(name).is_some();
        name.to_owned()
    });
    let (statement, rewritten) = statement(&sentence);
    Split {
        statement,
        body: verbatim(after),
        rewritten,
        metadata,
        modal,
    }
}

/// The statement a sentence gives: slots when it renders back from them, or when it does once
/// its first `SHALL` or `MUST` is written as `shall`, with the word written; otherwise the
/// sentence as text.
pub(super) fn statement(sentence: &str) -> (Statement, Option<&'static str>) {
    let verbatim = Statement::parse(sentence);
    if matches!(verbatim, Statement::Form(_)) {
        return (verbatim, None);
    }
    if let Some(range) = first_modal(sentence) {
        let rewritten = format!(
            "{}shall{}",
            &sentence[..range.start],
            &sentence[range.end..]
        );
        let parsed = Statement::parse(&rewritten);
        if matches!(parsed, Statement::Form(_)) {
            let word = if &sentence[range] == "MUST" {
                "MUST"
            } else {
                "SHALL"
            };
            return (parsed, Some(word));
        }
    }
    (Statement::Text(sentence.to_owned()), None)
}

/// The byte range of the first `SHALL` or `MUST` that stands as a word of its own.
pub(crate) fn first_modal(text: &str) -> Option<Range<usize>> {
    ["SHALL", "MUST"]
        .into_iter()
        .filter_map(|word| {
            text.match_indices(word)
                .map(|(start, _)| start..start + word.len())
                .find(|range| stands_alone(text, range))
        })
        .min_by_key(|range| range.start)
}

fn stands_alone(text: &str, range: &Range<usize>) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    !text[..range.start].chars().next_back().is_some_and(word)
        && !text[range.end..].chars().next().is_some_and(word)
}

/// The label and value of a metadata line: one that, trimmed, begins with a bold label followed
/// by a colon, such as `**ID**: AUTH-1`. The value is the text after the colon, trimmed.
pub(crate) fn metadata_entry(line: &str) -> Option<(&str, &str)> {
    let (label, after) = line.trim().strip_prefix("**")?.split_once("**")?;
    let value = after.strip_prefix(':')?;
    (!label.is_empty() && !label.contains('*')).then(|| (label, value.trim()))
}

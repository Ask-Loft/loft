//! A requirement's text split into the Requirement's statement and body, with the modal word
//! `SHALL` or `MUST` written as `shall` when that gives the statement slots.

use std::ops::Range;

use super::read::Source;
use crate::statement::Statement;

/// The statement and body a requirement's text gives.
#[derive(Debug)]
pub(super) struct Split {
    pub statement: Statement,
    pub body: Option<String>,
    /// Whether the modal word was written as `shall` to give the statement slots.
    pub rewritten: bool,
    /// The 1-based line of each metadata line.
    pub metadata: Vec<usize>,
    /// Whether the text, or the name when there is no text, holds `SHALL` or `MUST`.
    pub modal: bool,
}

/// Splits the text of a requirement named `name`.
pub(super) fn split(source: &Source, text: Range<usize>, name: &str) -> Split {
    let mut metadata = Vec::new();
    let mut metadata_lines = Vec::new();
    let mut rest = Vec::new();
    let mut modal = false;
    for (index, line, fenced) in source.lines(text) {
        if !fenced && is_metadata(line.trim()) {
            metadata.push(index + 1);
            metadata_lines.push(line.trim().to_owned());
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
    let body = [
        (!metadata_lines.is_empty()).then(|| metadata_lines.join("\n")),
        verbatim(after),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    Split {
        statement,
        body: (!body.is_empty()).then(|| body.join("\n\n")),
        rewritten,
        metadata,
        modal,
    }
}

/// The statement a sentence gives: slots when it renders back from them, or when it does once
/// its first `SHALL` or `MUST` is written as `shall`; otherwise the sentence as text.
pub(super) fn statement(sentence: &str) -> (Statement, bool) {
    let verbatim = Statement::parse(sentence);
    if matches!(verbatim, Statement::Form(_)) {
        return (verbatim, false);
    }
    if let Some(range) = first_modal(sentence) {
        let rewritten = format!(
            "{}shall{}",
            &sentence[..range.start],
            &sentence[range.end..]
        );
        let parsed = Statement::parse(&rewritten);
        if matches!(parsed, Statement::Form(_)) {
            return (parsed, true);
        }
    }
    (Statement::Text(sentence.to_owned()), false)
}

/// The byte range of the first `SHALL` or `MUST` that stands as a word of its own.
fn first_modal(text: &str) -> Option<Range<usize>> {
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

/// Whether a trimmed line begins with a bold label followed by a colon, such as `**ID**:`.
fn is_metadata(line: &str) -> bool {
    line.strip_prefix("**")
        .and_then(|rest| rest.split_once("**"))
        .is_some_and(|(label, after)| {
            !label.is_empty() && !label.contains('*') && after.starts_with(':')
        })
}

/// Lines as written, without leading and trailing blank lines.
fn verbatim(lines: &[&str]) -> Option<String> {
    let first = lines.iter().position(|line| !line.trim().is_empty())?;
    let last = lines.iter().rposition(|line| !line.trim().is_empty())?;
    Some(lines[first..=last].join("\n"))
}

//! The texts the language rules read (AC-646): a name or statement as its tokens, without its
//! code spans for the rules that read it as prose, and a body as
//! the text a Markdown reader shows, without code blocks, HTML blocks or code spans, each token
//! and sentence with the body line it starts on.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

use crate::text::{lowercase, token_ranges};

/// A body's shown text: its text pieces joined by single spaces, and where each piece began.
pub(super) struct Body {
    text: String,
    /// Each piece's offset in `text` and the body line it began on, in order.
    pieces: Vec<(usize, usize)>,
}

impl Body {
    pub(super) fn read(markdown: &str) -> Self {
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(markdown.match_indices('\n').map(|(at, _)| at + 1))
            .collect();
        let line_of = |offset: usize| line_starts.partition_point(|start| *start <= offset);
        let mut text = String::new();
        let mut pieces = Vec::new();
        for (event, range) in shown_events(markdown) {
            if let Event::Text(piece) = event {
                join(&mut text, &mut pieces, &piece, line_of(range.start));
            }
        }
        Self { text, pieces }
    }

    /// The body line of an offset into the shown text.
    fn line_at(&self, offset: usize) -> usize {
        let at = self.pieces.partition_point(|(start, _)| *start <= offset);
        at.checked_sub(1).map_or(1, |index| self.pieces[index].1)
    }

    /// Every token of the shown text, with the body line it is on.
    pub(super) fn tokens(&self) -> (Vec<String>, Vec<usize>) {
        token_ranges(&self.text)
            .into_iter()
            .map(|range| {
                (
                    lowercase(&self.text[range.clone()]),
                    self.line_at(range.start),
                )
            })
            .unzip()
    }

    /// The shown text's tokens, each token's body line, and whether each token follows the one
    /// before it across a single hyphen.
    pub(super) fn words(&self) -> (Vec<String>, Vec<usize>, Vec<bool>) {
        let ranges = token_ranges(&self.text);
        let lines = ranges
            .iter()
            .map(|range| self.line_at(range.start))
            .collect();
        let (tokens, joins) = words_of(&self.text, &ranges);
        (tokens, lines, joins)
    }

    /// Every sentence of the shown text, trimmed, with its tokens and the body line it starts on.
    pub(super) fn sentences(&self) -> Vec<(Vec<String>, usize)> {
        self.text
            .split_sentence_bound_indices()
            .map(|(start, sentence)| {
                let trimmed = sentence.trim_start();
                let start = start + sentence.len() - trimmed.len();
                (tokens(trimmed.trim_end()), self.line_at(start))
            })
            .filter(|(tokens, _)| !tokens.is_empty())
            .collect()
    }
}

/// The events of `markdown` a reader is shown, each with its byte range: everything but what
/// lies inside a code block or an HTML block, whose end is kept so that a reader can mark where
/// the block stood.
fn shown_events(markdown: &str) -> impl Iterator<Item = (Event<'_>, Range<usize>)> {
    let mut excluded = 0_u32;
    Parser::new_ext(markdown, Options::empty())
        .into_offset_iter()
        .filter(move |(event, _)| match event {
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) => {
                excluded += 1;
                false
            }
            Event::End(TagEnd::CodeBlock | TagEnd::HtmlBlock) => {
                excluded -= 1;
                true
            }
            _ => excluded == 0,
        })
}

/// The text a reader is shown of `markdown`, as one string: emphasis and links drop their
/// marks, so that `512 **MB**` reads `512 MB`; each code span stands as the one mark `\u{1}`,
/// which is neither a letter nor a digit; a line break is a space; and each paragraph, heading,
/// list item, code block and HTML block ends a line, as each paragraph, heading, list, list item
/// and block quote that starts after other text begins one, so that an item nested in a tight
/// list stands apart from the text before it.
pub(super) fn plain(markdown: &str) -> String {
    let mut text = String::new();
    for (event, _) in shown_events(markdown) {
        match event {
            Event::Start(
                Tag::Paragraph
                | Tag::Heading { .. }
                | Tag::List(_)
                | Tag::Item
                | Tag::BlockQuote(_),
            ) if !text.is_empty() && !text.ends_with('\n') => text.push('\n'),
            Event::Text(piece) => text.push_str(&piece),
            Event::Code(_) => text.push('\u{1}'),
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::CodeBlock
                | TagEnd::HtmlBlock,
            ) => text.push('\n'),
            _ => {}
        }
    }
    text
}

/// Appends one text piece, a space before it after the first.
fn join(text: &mut String, pieces: &mut Vec<(usize, usize)>, piece: &str, line: usize) {
    if !pieces.is_empty() {
        text.push(' ');
    }
    pieces.push((text.len(), line));
    text.push_str(piece);
}

/// The tokens of a name or statement.
pub(super) fn tokens(text: &str) -> Vec<String> {
    crate::text::tokenize(text)
}

/// The tokens of `text`, and whether each follows the one before it across a single hyphen, as
/// the words of `key-shape` do.
pub(super) fn words(text: &str) -> (Vec<String>, Vec<bool>) {
    words_of(text, &token_ranges(text))
}

/// The tokens at `ranges` of `text`, and whether each follows the one before it across a hyphen.
fn words_of(text: &str, ranges: &[std::ops::Range<usize>]) -> (Vec<String>, Vec<bool>) {
    let tokens = ranges
        .iter()
        .map(|range| lowercase(&text[range.clone()]))
        .collect();
    let mut joins = vec![false; ranges.len()];
    for (index, pair) in ranges.windows(2).enumerate() {
        joins[index + 1] = &text[pair[0].end..pair[1].start] == "-";
    }
    (tokens, joins)
}

/// The text of a name or statement with each code span replaced by a space.
pub(super) fn without_code_spans(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    let mut copied = 0;
    for span in crate::text::code_spans(text) {
        shown.push_str(&text[copied..span.start]);
        shown.push(' ');
        copied = span.end;
    }
    shown.push_str(&text[copied..]);
    shown
}

/// Where `phrase` starts as a run of `tokens`.
pub(super) fn phrase_positions(tokens: &[String], phrase: &[String]) -> Vec<usize> {
    if phrase.is_empty() || phrase.len() > tokens.len() {
        return Vec::new();
    }
    tokens
        .windows(phrase.len())
        .enumerate()
        .filter_map(|(index, window)| (window == phrase).then_some(index))
        .collect()
}

/// How many times the phrases of a lexicon occur in `tokens`, all together.
pub(super) fn occurrences(tokens: &[String], lexicon: &[Vec<String>]) -> usize {
    lexicon
        .iter()
        .map(|phrase| phrase_positions(tokens, phrase).len())
        .sum()
}

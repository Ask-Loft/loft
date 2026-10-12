//! The texts the language rules read (AC-646): a name or statement as its tokens, and a body as
//! the text a Markdown reader shows, without code blocks, HTML blocks or code spans, each token
//! and sentence with the body line it starts on.

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
        let mut excluded = 0_u32;
        let mut text = String::new();
        let mut pieces = Vec::new();
        for (event, range) in Parser::new_ext(markdown, Options::empty()).into_offset_iter() {
            match event {
                Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) => excluded += 1,
                Event::End(TagEnd::CodeBlock | TagEnd::HtmlBlock) => excluded -= 1,
                Event::Text(piece) if excluded == 0 => {
                    join(&mut text, &mut pieces, &piece, line_of(range.start));
                }
                _ => {}
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

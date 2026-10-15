//! A Purpose that is a placeholder rather than a Purpose anyone wrote, as `OpenSpec`'s validator
//! reads one: text outside fenced code that opens with a `TBD` or `TODO` marker, or that holds the
//! sentence `openspec archive` writes for a new capability.

use super::read::fence_mask;

/// The archive placeholder's opening, up to the change id.
const ARCHIVE_PREFIX: &str = "TBD - created by archiving change ";

/// The archive placeholder's closing, after the change id.
const ARCHIVE_SUFFIX: &str = ". Update Purpose after archive.";

const MARKERS: [&str; 2] = ["TBD", "TODO"];

/// Whether `text` is a placeholder: outside fenced code and trimmed, it opens with `TBD` or
/// `TODO` in upper case as a word, or in any case followed by spaces or tabs and then a line's
/// end or punctuation; or it holds the archive sentence.
pub(crate) fn is_placeholder(text: &str) -> bool {
    let prose = prose(text);
    opens(&prose) || holds_archive_sentence(&prose)
}

/// Whether `text`, outside fenced code and trimmed, opens with `TBD` or `TODO` as
/// [`is_placeholder`] reads it.
pub(crate) fn opens_with_marker(text: &str) -> bool {
    opens(&prose(text))
}

/// The lines of `text` outside fenced code, joined and trimmed.
fn prose(text: &str) -> String {
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let fenced = fence_mask(&lines);
    let kept: Vec<&str> = lines
        .iter()
        .zip(fenced)
        .filter(|(_, fenced)| !fenced)
        .map(|(line, _)| line.as_str())
        .collect();
    kept.join("\n").trim().to_owned()
}

fn opens(prose: &str) -> bool {
    MARKERS.iter().any(|marker| {
        let Some(head) = prose.get(..marker.len()) else {
            return false;
        };
        let rest = &prose[marker.len()..];
        if rest.chars().next().is_some_and(continues_word) {
            return false;
        }
        head == *marker || (head.eq_ignore_ascii_case(marker) && punctuated(rest))
    })
}

/// Whether a character would make the marker part of a longer word.
fn continues_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Whether `rest` is spaces or tabs and then a line's end or punctuation.
fn punctuated(rest: &str) -> bool {
    let rest = rest.trim_start_matches([' ', '\t']);
    rest.chars().next().is_none_or(|next| {
        matches!(
            next,
            '\n' | ':'
                | '-'
                | '\u{2013}'
                | '\u{2014}'
                | '.'
                | ','
                | ';'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
        )
    })
}

fn holds_archive_sentence(prose: &str) -> bool {
    prose
        .match_indices(ARCHIVE_SUFFIX)
        .any(|(suffix, _)| prose[..suffix].contains(ARCHIVE_PREFIX))
}

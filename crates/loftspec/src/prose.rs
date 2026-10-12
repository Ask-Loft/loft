//! What the Format reads in prose: the label references a name or body makes (D-11 RQ-53,
//! RQ-102) and the Markdown heading lines in a body (RQ-54, RQ-100).

use std::ops::Range;

use crate::identity::Label;

/// The label references in `text`, in order: `P-n`, `FG-n`, `F-n`, `RQ-n`, `AC-n`, `CMP-n`,
/// `CON-n`, `QA-n` and `ADR-n`, each optionally after a document prefix `D-n/` (AC-188,
/// AC-189). A reference stands on its own: it neither continues a word, a number or another
/// label, nor runs into a letter, a digit or a decimal point followed by a digit.
#[must_use]
pub fn label_references(text: &str) -> Vec<Label> {
    label_spans(text)
        .into_iter()
        .map(|span| span.label)
        .collect()
}

/// A label reference in a text: where the label stands, the label, and whether a document
/// prefix `D-n/` qualifies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSpan {
    /// The bytes of the label itself, without its document prefix.
    pub range: Range<usize>,
    pub label: Label,
    pub qualified: bool,
}

/// The label references in `text`, in order, as [`label_references`] finds them, each with
/// where it stands.
#[must_use]
pub fn label_spans(text: &str) -> Vec<LabelSpan> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        let starts_word = position == 0 || !continues_word(bytes[position - 1]);
        if starts_word && bytes[position].is_ascii_uppercase() {
            let start = after_document_prefix(bytes, position);
            if let Some((label, end)) = label_at(text, start) {
                spans.push(LabelSpan {
                    range: start..end,
                    label,
                    qualified: start != position,
                });
                position = end;
                continue;
            }
        }
        position += 1;
    }
    spans
}

/// A byte that a reference cannot follow: letters, digits, `_`, `-` and `/`.
const fn continues_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'/')
}

/// The position after `D-n/` when `position` starts one, `position` otherwise.
fn after_document_prefix(bytes: &[u8], position: usize) -> usize {
    let rest = &bytes[position..];
    let Some(digits) = rest.strip_prefix(b"D-") else {
        return position;
    };
    let count = digits
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if count > 0 && digits.get(count) == Some(&b'/') {
        position + 2 + count + 1
    } else {
        position
    }
}

/// The label that starts at `start` and the position after it, if it stands on its own.
fn label_at(text: &str, start: usize) -> Option<(Label, usize)> {
    let bytes = text.as_bytes();
    let letters = bytes[start..]
        .iter()
        .take_while(|byte| byte.is_ascii_uppercase())
        .count();
    let hyphen = start + letters;
    if letters == 0 || bytes.get(hyphen) != Some(&b'-') {
        return None;
    }
    let digits = bytes[hyphen + 1..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    let end = hyphen + 1 + digits;
    let runs_on = match bytes.get(end) {
        Some(byte) if byte.is_ascii_alphanumeric() || *byte == b'_' => true,
        Some(b'.') => bytes.get(end + 1).is_some_and(u8::is_ascii_digit),
        _ => false,
    };
    if digits == 0 || runs_on {
        return None;
    }
    Label::parse(&text[start..end])
        .ok()
        .map(|label| (label, end))
}

/// The 1-based numbers of the lines that open a Markdown heading in `body`, outside fenced code
/// blocks: an ATX heading (up to three spaces, one to six `#`, then a space, a tab or the end of
/// the line) or the text line of a setext heading (a paragraph line underlined by `=` or `-`).
#[must_use]
pub fn heading_lines(body: &str) -> Vec<usize> {
    let mut headings = Vec::new();
    let mut fence: Option<&str> = None;
    let mut previous_is_text = false;
    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        if let Some(marker) = fence {
            if indent < 4 && trimmed.starts_with(marker) {
                fence = None;
            }
            previous_is_text = false;
            continue;
        }
        if indent < 4 && (trimmed.starts_with("```") || trimmed.starts_with("~~~")) {
            fence = Some(&trimmed[..3]);
            previous_is_text = false;
            continue;
        }
        if indent < 4 && is_atx_heading(trimmed) {
            headings.push(index + 1);
            previous_is_text = false;
            continue;
        }
        if previous_is_text && indent < 4 && is_setext_underline(trimmed) {
            headings.push(index);
            previous_is_text = false;
            continue;
        }
        previous_is_text = !line.trim().is_empty() && indent < 4;
    }
    headings
}

fn is_atx_heading(line: &str) -> bool {
    let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
    (1..=6).contains(&hashes) && matches!(line.as_bytes().get(hashes), None | Some(b' ' | b'\t'))
}

fn is_setext_underline(line: &str) -> bool {
    let line = line.trim_end();
    !line.is_empty()
        && (line.bytes().all(|byte| byte == b'=') || line.bytes().all(|byte| byte == b'-'))
}

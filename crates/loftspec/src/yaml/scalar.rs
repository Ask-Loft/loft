//! A value on the line of its key or sequence entry: a plain or double-quoted scalar, the header
//! of a literal block, or nothing. Every other indicator is refused at its position.

use super::error::{Error, ErrorKind};
use super::source::Line;
use super::{Scalar, Style};

/// How a literal block treats its final line feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Chomping {
    /// `|-`: no final line feed.
    Strip,
    /// `|`: one final line feed.
    Clip,
    /// `|+`: every final line feed.
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BlockHeader {
    pub chomping: Chomping,
    /// The content's indentation relative to its parent, when the header states it.
    pub indentation: Option<usize>,
}

/// What follows a key's colon or an entry's dash on the same line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Inline {
    Scalar(Scalar),
    Literal(BlockHeader),
    Nothing,
}

/// Reads the value that starts `offset` bytes into `line.text`.
pub(super) fn inline(line: &Line<'_>, offset: usize) -> Result<Inline, Error> {
    let text = &line.text[offset..];
    let error = |at: usize, kind| Err(Error::at(line.position(offset + at), kind));
    let Some(first) = text.chars().next() else {
        return Ok(Inline::Nothing);
    };
    let separated = text[1..].is_empty() || text[1..].starts_with([' ', '\t']);
    match first {
        '"' => double_quoted(line, offset).map(Inline::Scalar),
        '|' => block_header(line, offset + 1).map(Inline::Literal),
        '>' => error(0, ErrorKind::FoldedScalar),
        '\'' => error(0, ErrorKind::SingleQuotedScalar),
        '&' => error(0, ErrorKind::Anchor),
        '*' => error(0, ErrorKind::Alias),
        '!' => error(0, ErrorKind::Tag),
        '{' | '[' | '}' | ']' | ',' => error(0, ErrorKind::FlowCollection),
        '#' => error(0, ErrorKind::Comment),
        '%' | '@' | '`' => error(0, ErrorKind::ReservedIndicator(first)),
        '-' if separated => error(0, ErrorKind::UnexpectedSequence),
        '?' | ':' if separated => error(0, ErrorKind::ComplexKey),
        _ => plain(line, offset).map(Inline::Scalar),
    }
}

/// A plain scalar: the rest of the line without its trailing white space. A ` #` starts a
/// comment and a `: ` a mapping, both refused.
fn plain(line: &Line<'_>, offset: usize) -> Result<Scalar, Error> {
    let text = &line.text[offset..];
    let error = |at: usize, kind| Err(Error::at(line.position(offset + at), kind));
    let bytes = text.as_bytes();
    for (index, pair) in bytes.windows(2).enumerate() {
        match pair {
            [b' ' | b'\t', b'#'] => return error(index + 1, ErrorKind::Comment),
            [b':', b' ' | b'\t'] => return error(index, ErrorKind::UnexpectedMapping),
            _ => {}
        }
    }
    if text.ends_with(':') {
        return error(text.len() - 1, ErrorKind::UnexpectedMapping);
    }
    Ok(Scalar {
        style: Style::Plain,
        text: text.trim_end_matches([' ', '\t']).to_owned(),
    })
}

/// A double-quoted scalar closed on its own line (no line folding), with its escapes read.
fn double_quoted(line: &Line<'_>, offset: usize) -> Result<Scalar, Error> {
    let text = &line.text[offset..];
    let mut value = String::new();
    let mut characters = text.char_indices().skip(1);
    while let Some((index, character)) = characters.next() {
        match character {
            '"' => {
                after_value(line, offset + index + 1)?;
                return Ok(Scalar {
                    style: Style::DoubleQuoted,
                    text: value,
                });
            }
            '\\' => value.push(escape(&mut characters).ok_or_else(|| {
                Error::at(line.position(offset + index), ErrorKind::InvalidEscape)
            })?),
            other => value.push(other),
        }
    }
    Err(Error::at(line.position(offset), ErrorKind::UnclosedQuote))
}

/// The character an escape stands for, reading the characters after its backslash.
fn escape(characters: &mut impl Iterator<Item = (usize, char)>) -> Option<char> {
    let (_, code) = characters.next()?;
    let digits = match code {
        '0' => return Some('\0'),
        'a' => return Some('\u{07}'),
        'b' => return Some('\u{08}'),
        't' | '\t' => return Some('\t'),
        'n' => return Some('\n'),
        'v' => return Some('\u{0B}'),
        'f' => return Some('\u{0C}'),
        'r' => return Some('\r'),
        'e' => return Some('\u{1B}'),
        ' ' | '"' | '/' | '\\' => return Some(code),
        'N' => return Some('\u{85}'),
        '_' => return Some('\u{A0}'),
        'L' => return Some('\u{2028}'),
        'P' => return Some('\u{2029}'),
        'x' => 2,
        'u' => 4,
        'U' => 8,
        _ => return None,
    };
    let hex: String = characters.take(digits).map(|(_, digit)| digit).collect();
    if hex.len() != digits || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
}

/// The header of a literal block, `offset` bytes into the line just past its `|`: at most one
/// chomping indicator and one indentation digit, in either order.
fn block_header(line: &Line<'_>, offset: usize) -> Result<BlockHeader, Error> {
    let mut header = BlockHeader {
        chomping: Chomping::Clip,
        indentation: None,
    };
    let mut chomping_seen = false;
    let mut end = offset;
    for character in line.text[offset..].chars() {
        match character {
            '-' | '+' if !chomping_seen => {
                chomping_seen = true;
                header.chomping = if character == '-' {
                    Chomping::Strip
                } else {
                    Chomping::Keep
                };
            }
            '1'..='9' if header.indentation.is_none() => {
                header.indentation = character.to_digit(10).map(|digit| digit as usize);
            }
            _ => break,
        }
        end += 1;
    }
    after_value(line, end).map_err(|error| match error.kind {
        ErrorKind::TrailingText => Error::at(line.position(end), ErrorKind::InvalidBlockHeader),
        _ => error,
    })?;
    Ok(header)
}

/// Requires nothing but spaces after a closed value: a `#` there is a comment.
fn after_value(line: &Line<'_>, offset: usize) -> Result<(), Error> {
    let rest = &line.text[offset..];
    let trimmed = rest.trim_start_matches([' ', '\t']);
    let at = offset + (rest.len() - trimmed.len());
    match trimmed.chars().next() {
        None => Ok(()),
        Some('#') if at > offset => Err(Error::at(line.position(at), ErrorKind::Comment)),
        Some(_) => Err(Error::at(line.position(at), ErrorKind::TrailingText)),
    }
}

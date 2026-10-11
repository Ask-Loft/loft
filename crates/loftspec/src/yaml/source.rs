//! The text of a file before structure: UTF-8 (RQ-65), no byte order mark (AC-231), lines ended
//! by a line feed alone, and only printable characters.

use super::error::{Error, ErrorKind, Location, Position};

/// Whether a character may appear in a file as itself (RQ-35's printable set): a tab, a line
/// feed, or a character in U+0020..U+007E, U+00A0..U+D7FF, U+E000..U+FFFD or
/// U+10000..U+10FFFF other than U+2028, U+2029 and U+FEFF.
#[must_use]
pub fn is_printable(character: char) -> bool {
    matches!(character,
        '\u{20}'..='\u{7E}'
        | '\u{A0}'..='\u{D7FF}'
        | '\u{E000}'..='\u{FFFD}'
        | '\u{10000}'..='\u{10FFFF}')
        && !matches!(character, '\u{2028}' | '\u{2029}' | '\u{FEFF}')
}

/// The file's text, refused at the first byte that is not UTF-8, at a byte order mark, at a
/// carriage return or at a character that is not printable.
pub(super) fn decode(bytes: &[u8]) -> Result<&str, Error> {
    let text = std::str::from_utf8(bytes).map_err(|error| Error {
        location: Location::Byte(error.valid_up_to()),
        kind: ErrorKind::InvalidUtf8,
    })?;
    let mut line = 1;
    let mut column = 1;
    for character in text.chars() {
        let position = Position::new(line, column);
        match character {
            '\n' => {
                line += 1;
                column = 1;
                continue;
            }
            '\t' => {}
            '\u{FEFF}' => return Err(Error::at(position, ErrorKind::ByteOrderMark)),
            '\r' => return Err(Error::at(position, ErrorKind::CarriageReturn)),
            other if !is_printable(other) => {
                return Err(Error::at(position, ErrorKind::NotPrintable(other.into())));
            }
            _ => {}
        }
        column += 1;
    }
    Ok(text)
}

/// One line of a file: its number, its leading spaces and the text after them.
#[derive(Debug, Clone, Copy)]
pub(super) struct Line<'a> {
    pub number: usize,
    /// The column of `text`'s first character; for a line read from the file, one more than
    /// its leading spaces.
    pub column: usize,
    /// The leading spaces, or for a line that continues a sequence entry, the entry's
    /// indentation.
    pub indent: usize,
    pub text: &'a str,
    /// The whole line as the file holds it, for literal block content.
    pub raw: &'a str,
}

impl<'a> Line<'a> {
    /// The lines of `text`, numbered from 1.
    pub fn split(text: &'a str) -> Vec<Self> {
        let mut lines: Vec<Self> = text
            .split('\n')
            .enumerate()
            .map(|(index, raw)| {
                let indent = raw.len() - raw.trim_start_matches(' ').len();
                Self {
                    number: index + 1,
                    column: indent + 1,
                    indent,
                    text: &raw[indent..],
                    raw,
                }
            })
            .collect();
        // A file that ends with a line feed has no line after it.
        if lines.last().is_some_and(|line| line.raw.is_empty()) {
            lines.pop();
        }
        lines
    }

    /// Whether the line holds nothing but spaces.
    pub const fn is_blank(&self) -> bool {
        self.text.is_empty()
    }

    /// The position of the character `offset` bytes into `text`.
    pub fn position(&self, offset: usize) -> Position {
        Position::new(
            self.number,
            self.column + self.text[..offset].chars().count(),
        )
    }

    /// The position of the line's first character after its indentation.
    pub fn start(&self) -> Position {
        self.position(0)
    }
}

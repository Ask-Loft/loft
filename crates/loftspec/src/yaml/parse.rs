//! Block structure by indentation: mappings of plain field names and sequences of entries, each
//! value on its key's line, beneath it, or as a literal block.

use std::collections::BTreeMap;

use super::error::{Error, ErrorKind, Position};
use super::scalar::{self, BlockHeader, Chomping, Inline};
use super::source::Line;
use super::{Entry, Node, Scalar, Style, Value};

/// Reads a whole file: nothing, or one mapping or sequence starting at column 1.
pub(super) fn document(text: &str) -> Result<Option<Node>, Error> {
    let mut parser = Parser {
        lines: Line::split(text),
        next: 0,
    };
    let Some(first) = parser.peek()? else {
        return Ok(None);
    };
    if first.indent != 0 {
        return Err(Error::at(first.start(), ErrorKind::UnexpectedIndentation));
    }
    let node = parser.block(0)?;
    match parser.peek()? {
        None => Ok(Some(node)),
        Some(line) if line.indent > 0 => {
            Err(Error::at(line.start(), ErrorKind::UnexpectedIndentation))
        }
        Some(line) if is_entry(line.text) => {
            Err(Error::at(line.start(), ErrorKind::UnexpectedSequence))
        }
        Some(line) => Err(Error::at(line.start(), ErrorKind::UnexpectedMapping)),
    }
}

struct Parser<'a> {
    lines: Vec<Line<'a>>,
    next: usize,
}

/// Whether a line's text starts a sequence entry.
fn is_entry(text: &str) -> bool {
    text == "-" || text.starts_with("- ") || text.starts_with("-\t")
}

impl<'a> Parser<'a> {
    /// The next line that is not blank, checked for what no structural line may hold.
    fn peek(&mut self) -> Result<Option<Line<'a>>, Error> {
        while self.lines.get(self.next).is_some_and(Line::is_blank) {
            self.next += 1;
        }
        let Some(line) = self.lines.get(self.next).copied() else {
            return Ok(None);
        };
        let marker = |prefix: &str| {
            line.text
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']))
        };
        let kind = if line.text.starts_with('\t') {
            ErrorKind::TabInIndentation
        } else if line.text.starts_with('#') {
            ErrorKind::Comment
        } else if line.column == 1 && (marker("---") || marker("...") || line.text.starts_with('%'))
        {
            ErrorKind::DocumentMarker
        } else {
            return Ok(Some(line));
        };
        Err(Error::at(line.start(), kind))
    }

    /// The mapping or sequence whose first line is the next one, at `indent`.
    fn block(&mut self, indent: usize) -> Result<Node, Error> {
        match self.peek()? {
            Some(line) if is_entry(line.text) => self.sequence(indent),
            _ => self.mapping(indent),
        }
    }

    fn mapping(&mut self, indent: usize) -> Result<Node, Error> {
        let mut entries = Vec::new();
        let mut seen = BTreeMap::new();
        let mut position = None;
        while let Some(line) = self.peek()? {
            if line.indent < indent {
                break;
            }
            if line.indent > indent {
                return Err(Error::at(line.start(), ErrorKind::UnexpectedIndentation));
            }
            if is_entry(line.text) {
                return Err(Error::at(line.start(), ErrorKind::UnexpectedSequence));
            }
            let (key, offset) = field(&line)?;
            if seen.insert(key.to_owned(), line.start()).is_some() {
                return Err(Error::at(
                    line.start(),
                    ErrorKind::DuplicateKey(key.to_owned()),
                ));
            }
            position.get_or_insert(line.start());
            self.next += 1;
            let value = self.value(&line, offset, indent, true)?;
            entries.push(Entry {
                key: key.to_owned(),
                key_position: line.start(),
                value,
            });
        }
        Ok(Node {
            position: position.unwrap_or(Position::new(1, 1)),
            value: Value::Mapping(entries),
        })
    }

    fn sequence(&mut self, indent: usize) -> Result<Node, Error> {
        let mut items = Vec::new();
        let mut position = None;
        while let Some(line) = self.peek()? {
            if line.indent > indent {
                return Err(Error::at(line.start(), ErrorKind::UnexpectedIndentation));
            }
            if line.indent < indent || !is_entry(line.text) {
                break;
            }
            position.get_or_insert(line.start());
            self.next += 1;
            items.push(self.entry(line, indent)?);
        }
        Ok(Node {
            position: position.unwrap_or(Position::new(1, 1)),
            value: Value::Sequence(items),
        })
    }

    /// One sequence entry: a value on the dash's line, a mapping that starts there, or a block
    /// beneath it.
    fn entry(&mut self, line: Line<'a>, indent: usize) -> Result<Node, Error> {
        let rest = &line.text[1..];
        if rest.starts_with('\t') {
            return Err(Error::at(line.position(1), ErrorKind::TabSeparator));
        }
        let offset = 1 + (rest.len() - rest.trim_start_matches(' ').len());
        let content = &line.text[offset..];
        if content.is_empty() {
            return self.value(&line, offset, indent, false);
        }
        if is_entry(content) {
            return Err(Error::at(line.position(offset), ErrorKind::CompactSequence));
        }
        if !starts_field(content) {
            return self.value(&line, offset, indent, false);
        }
        // The mapping's first field shares the dash's line: read that line again as if it
        // began at the field.
        self.next -= 1;
        self.lines[self.next] = Line {
            column: line.column + offset,
            indent: indent + offset,
            text: content,
            ..line
        };
        self.mapping(indent + offset)
    }

    /// The value after a key or a dash at `offset` on `line`, whose parent is at `indent`. Under
    /// a key, a sequence may start at the key's own indentation.
    fn value(
        &mut self,
        line: &Line<'a>,
        offset: usize,
        indent: usize,
        under_key: bool,
    ) -> Result<Node, Error> {
        let position = line.position(offset);
        let value = match scalar::inline(line, offset)? {
            Inline::Scalar(scalar) => {
                self.no_continuation(indent, scalar.style)?;
                Value::Scalar(scalar)
            }
            Inline::Literal(header) => Value::Scalar(self.literal(indent, header)?),
            Inline::Nothing => match self.peek()? {
                Some(next) if next.indent > indent => return self.block(next.indent),
                Some(next) if under_key && next.indent == indent && is_entry(next.text) => {
                    return self.sequence(indent);
                }
                _ => Value::Empty,
            },
        };
        Ok(Node { position, value })
    }

    /// Refuses a line indented under a scalar that ended on its key's line.
    fn no_continuation(&mut self, indent: usize, style: Style) -> Result<(), Error> {
        match self.peek()? {
            Some(next) if next.indent > indent => {
                let kind = if style == Style::Plain {
                    ErrorKind::MultiLinePlainScalar
                } else {
                    ErrorKind::UnexpectedIndentation
                };
                Err(Error::at(next.start(), kind))
            }
            _ => Ok(()),
        }
    }

    /// The content of a literal block whose parent is at `indent`, its lines taken byte for
    /// byte after the content's indentation.
    fn literal(&mut self, indent: usize, header: BlockHeader) -> Result<Scalar, Error> {
        let content_indent = match header.indentation {
            Some(relative) => indent + relative,
            None => self.detect_indentation(indent)?,
        };
        let mut lines: Vec<&str> = Vec::new();
        while let Some(line) = self.lines.get(self.next) {
            let raw = line.raw;
            let spaces = raw.len() - raw.trim_start_matches(' ').len();
            if spaces == raw.len() {
                lines.push(raw.get(content_indent..).unwrap_or(""));
            } else if spaces >= content_indent {
                lines.push(&raw[content_indent..]);
            } else {
                break;
            }
            self.next += 1;
        }
        Ok(Scalar {
            style: Style::Literal,
            text: chomp(&lines, header.chomping),
        })
    }

    /// The indentation of a literal block's first line that is not empty; a leading empty line
    /// indented past it is refused.
    fn detect_indentation(&self, indent: usize) -> Result<usize, Error> {
        let leading = |line: &Line<'_>| line.raw.len() - line.raw.trim_start_matches(' ').len();
        let rest = &self.lines[self.next..];
        let Some(first) = rest.iter().position(|line| leading(line) < line.raw.len()) else {
            return Ok(indent + 1);
        };
        let spaces = leading(&rest[first]);
        if spaces <= indent {
            return Ok(indent + 1);
        }
        match rest[..first].iter().find(|line| line.raw.len() > spaces) {
            Some(empty) => Err(Error::at(
                Position::new(empty.number, spaces + 1),
                ErrorKind::OverIndentedEmptyLine,
            )),
            None => Ok(spaces),
        }
    }
}

/// The text of a literal block from its content lines, its final line feeds as its chomping
/// indicator says.
fn chomp(lines: &[&str], chomping: Chomping) -> String {
    let last = lines.iter().rposition(|line| !line.is_empty());
    let (body, trailing) = match last {
        Some(index) => (&lines[..=index], lines.len() - index - 1),
        None => (&lines[..0], lines.len()),
    };
    let mut text = body.join("\n");
    let final_feeds = match (chomping, body.is_empty()) {
        (Chomping::Strip, _) | (Chomping::Clip, true) => 0,
        (Chomping::Clip, false) => 1,
        (Chomping::Keep, false) => 1 + trailing,
        (Chomping::Keep, true) => trailing,
    };
    text.extend(std::iter::repeat_n('\n', final_feeds));
    text
}

/// Whether `text` starts with a field name and its colon.
fn starts_field(text: &str) -> bool {
    let name = text.trim_start_matches(|character: char| character.is_ascii_alphanumeric());
    name.len() < text.len()
        && name.starts_with(':')
        && (name.len() == 1 || name[1..].starts_with([' ', '\t']))
}

/// The field name of a mapping line and the offset of its value: a letter followed by letters
/// and digits, a colon, then the end of the line or spaces.
fn field<'t>(line: &Line<'t>) -> Result<(&'t str, usize), Error> {
    let text = line.text;
    let error = |at: usize, kind| Err(Error::at(line.position(at), kind));
    if text.starts_with("? ") || text == "?" {
        return error(0, ErrorKind::ComplexKey);
    }
    let Some(colon) = text.find(':') else {
        return error(0, ErrorKind::InvalidKey(text.to_owned()));
    };
    let key = &text[..colon];
    let valid = key.starts_with(|character: char| character.is_ascii_alphabetic())
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric());
    let rest = &text[colon + 1..];
    if !valid || !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return error(0, ErrorKind::InvalidKey(key.to_owned()));
    }
    if rest.starts_with('\t') {
        return error(colon + 1, ErrorKind::TabSeparator);
    }
    Ok((
        key,
        colon + 1 + (rest.len() - rest.trim_start_matches(' ').len()),
    ))
}

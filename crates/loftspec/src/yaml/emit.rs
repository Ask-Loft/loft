//! The canonical writer (RQ-34, RQ-35, RQ-69): two-space indentation, a sequence entry two
//! spaces under its key, and each scalar in the one style its value allows.

use std::fmt::Write as _;

use super::error::Position;
use super::source::is_printable;

/// A value as the writer lays it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    /// Fields in the order written. An empty mapping as a field's value is the caller's to
    /// omit; at the top level it is an empty file.
    Mapping(Vec<(&'static str, Out)>),
    /// A mapping whose keys are data, such as extension data's: each key a letter followed by
    /// letters and digits, as a field's name is.
    Map(Vec<(String, Out)>),
    Sequence(Vec<Out>),
    /// A key, label, slug, enumerated value, name or identifier: plain unless a YAML reader
    /// would take it for a null, a boolean or a number.
    Token(String),
    /// An integer field, always plain.
    Integer(u64),
    /// A value written plain whatever it is, such as a signed integer or a boolean.
    Plain(String),
    /// Free text: a literal block where it reads back exactly, double-quoted otherwise.
    Text(String),
    /// YAML's `null`, written plain.
    Null,
}

impl Out {
    fn is_collection(&self) -> bool {
        matches!(self, Self::Mapping(_) | Self::Map(_) | Self::Sequence(_))
    }
}

/// The file text for `value`: nothing for an empty top-level collection.
#[must_use]
pub fn write(value: &Out) -> String {
    emit(value, &mut None)
}

/// The file text for `value`, as [`write`] gives it, with the text and the position of the value
/// of every field named `key` of a mapping of fields, in the order written: where a reader of the
/// text finds each key.
#[must_use]
pub(crate) fn write_keyed(value: &Out) -> (String, Vec<(String, Position)>) {
    let mut keys = Some(Keys::default());
    let text = emit(value, &mut keys);
    (text, keys.map(|keys| keys.found).unwrap_or_default())
}

/// The positions of the keys written so far, and how far the text is counted.
#[derive(Default)]
struct Keys {
    found: Vec<(String, Position)>,
    counted: usize,
    /// The lines before the line being written.
    lines: usize,
    line_start: usize,
}

impl Keys {
    /// Records `key`, whose value starts where the text now ends.
    fn found(&mut self, text: &str, key: &str) {
        for (offset, byte) in text.bytes().enumerate().skip(self.counted) {
            if byte == b'\n' {
                self.lines += 1;
                self.line_start = offset + 1;
            }
        }
        self.counted = text.len();
        let column = text[self.line_start..].chars().count() + 1;
        self.found
            .push((key.to_owned(), Position::new(self.lines + 1, column)));
    }
}

/// The text for `value`, recording the keys in `keys` where it is given.
fn emit(value: &Out, keys: &mut Option<Keys>) -> String {
    let mut text = String::new();
    match value {
        Out::Mapping(fields) => mapping(&mut text, fields, 0, false, keys),
        Out::Map(fields) => mapping(&mut text, fields, 0, false, &mut None),
        Out::Sequence(items) => sequence(&mut text, items, 0, keys),
        scalar => {
            line_value(&mut text, scalar, 0);
        }
    }
    text
}

fn indent(text: &mut String, width: usize) {
    text.extend(std::iter::repeat_n(' ', width));
}

/// A mapping at `width`; one that is a sequence entry starts on the dash's line. The value of a
/// field named `key` is recorded in `keys`; a mapping whose keys are data records none, and
/// passes none on.
fn mapping<K: AsRef<str>>(
    text: &mut String,
    fields: &[(K, Out)],
    width: usize,
    after_dash: bool,
    keys: &mut Option<Keys>,
) {
    for (index, (key, value)) in fields.iter().enumerate() {
        if index > 0 || !after_dash {
            indent(text, width);
        }
        text.push_str(key.as_ref());
        text.push(':');
        if value.is_collection() {
            text.push('\n');
            nested(text, value, width + 2, keys);
        } else {
            text.push(' ');
            if let (Some(found), "key", Out::Token(token)) = (keys.as_mut(), key.as_ref(), value) {
                found.found(text, token);
            }
            line_value(text, value, width);
        }
    }
}

fn sequence(text: &mut String, items: &[Out], width: usize, keys: &mut Option<Keys>) {
    for item in items {
        indent(text, width);
        text.push_str("- ");
        match item {
            Out::Mapping(fields) => mapping(text, fields, width + 2, true, keys),
            Out::Map(fields) => mapping(text, fields, width + 2, true, &mut None),
            Out::Sequence(_) => {
                text.pop();
                text.push('\n');
                nested(text, item, width + 2, keys);
            }
            scalar => line_value(text, scalar, width),
        }
    }
}

fn nested(text: &mut String, value: &Out, width: usize, keys: &mut Option<Keys>) {
    match value {
        Out::Mapping(fields) => mapping(text, fields, width, false, keys),
        Out::Map(fields) => mapping(text, fields, width, false, &mut None),
        Out::Sequence(items) => sequence(text, items, width, keys),
        _ => unreachable!("only collections nest"),
    }
}

/// A scalar after `key: ` or `- `, ending its line; a literal block's content follows,
/// indented two spaces past `width`.
fn line_value(text: &mut String, value: &Out, width: usize) {
    match value {
        Out::Integer(number) => {
            let _ = write!(text, "{number}");
        }
        Out::Null => text.push_str("null"),
        Out::Plain(value) => text.push_str(value),
        Out::Token(token) if is_plain(token) => text.push_str(token),
        Out::Token(token) => double_quoted(text, token),
        Out::Text(string) if is_literal(string) => literal(text, string, width + 2),
        Out::Text(string) => double_quoted(text, string),
        Out::Mapping(_) | Out::Map(_) | Out::Sequence(_) => {
            unreachable!("collections are not line values")
        }
    }
    text.push('\n');
}

/// Whether a string reads back byte for byte as a literal block (AC-121): it holds a line
/// feed and is not only line feeds, every character is printable or a tab, and its first line
/// that is not empty does not begin with a space.
fn is_literal(string: &str) -> bool {
    string.contains('\n')
        && string.chars().any(|character| character != '\n')
        && string
            .chars()
            .all(|character| matches!(character, '\n' | '\t') || is_printable(character))
        && !string
            .split('\n')
            .find(|line| !line.is_empty())
            .is_some_and(|line| line.starts_with(' '))
}

fn literal(text: &mut String, string: &str, width: usize) {
    let body = string.trim_end_matches('\n');
    let final_feeds = string.len() - body.len();
    text.push_str(match final_feeds {
        0 => "|-",
        1 => "|",
        _ => "|+",
    });
    for line in body.split('\n') {
        text.push('\n');
        if !line.is_empty() {
            indent(text, width);
            text.push_str(line);
        }
    }
    text.extend(std::iter::repeat_n('\n', final_feeds.saturating_sub(1)));
}

/// A double-quoted string on one line with the Format's escapes (RQ-35).
fn double_quoted(text: &mut String, string: &str) {
    text.push('"');
    for character in string.chars() {
        match character {
            '\\' => text.push_str("\\\\"),
            '"' => text.push_str("\\\""),
            '\n' => text.push_str("\\n"),
            '\t' => text.push_str("\\t"),
            '\r' => text.push_str("\\r"),
            '\0' => text.push_str("\\0"),
            other if is_printable(other) => text.push(other),
            other => {
                let _ = write!(text, "\\u{:04X}", u32::from(other));
            }
        }
    }
    text.push('"');
}

/// Whether a token is written plain: it is a well-formed plain scalar that a YAML 1.1 or 1.2
/// reader takes for a string, not for a null, a boolean or a number.
fn is_plain(token: &str) -> bool {
    let well_formed = token
        .chars()
        .next()
        .is_some_and(|first| !"-?:,[]{}#&*!|>'\"%@` ".contains(first))
        && !token.ends_with([' ', ':'])
        && !token.contains(": ")
        && !token.contains(" #")
        && token.chars().all(is_printable);
    well_formed && !is_null_or_boolean(token) && !is_number(token)
}

fn is_null_or_boolean(token: &str) -> bool {
    const WORDS: [&str; 13] = [
        "~", "null", "true", "false", "y", "n", "yes", "no", "on", "off", "", "nan", "inf",
    ];
    let word = token.trim_start_matches('.');
    WORDS.iter().any(|known| known.eq_ignore_ascii_case(word))
}

/// Whether a YAML 1.1 or 1.2 reader would read the token as an integer or a float, including
/// 1.1's underscores, prefixed bases and sexagesimal numbers.
fn is_number(token: &str) -> bool {
    let unsigned = token.strip_prefix(['+', '-']).unwrap_or(token);
    let digits = |text: &str, radix: u32| {
        !text.is_empty()
            && text.chars().any(|character| character.is_digit(radix))
            && text
                .chars()
                .all(|character| character == '_' || character.is_digit(radix))
    };
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if unsigned
            .strip_prefix(prefix)
            .is_some_and(|rest| digits(rest, radix))
        {
            return true;
        }
    }
    let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, Some(exponent)),
        None => (unsigned, None),
    };
    let exponent_ok = exponent
        .is_none_or(|exponent| digits(exponent.strip_prefix(['+', '-']).unwrap_or(exponent), 10));
    let mantissa_ok = mantissa.split(':').all(|part| {
        part.split('.')
            .all(|piece| piece.is_empty() || digits(piece, 10))
    }) && mantissa.chars().any(|character| character.is_ascii_digit())
        && mantissa.matches('.').count() <= 1;
    exponent_ok && mantissa_ok
}

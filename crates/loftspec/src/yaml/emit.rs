//! The canonical writer (RQ-34, RQ-35, RQ-69): two-space indentation, a sequence entry two
//! spaces under its key, and each scalar in the one style its value allows.

use std::fmt::Write as _;

use super::source::is_printable;

/// A value as the writer lays it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    /// Fields in the order written. An empty mapping as a field's value is the caller's to
    /// omit; at the top level it is an empty file.
    Mapping(Vec<(&'static str, Out)>),
    Sequence(Vec<Out>),
    /// A key, label, slug, enumerated value, name or identifier: plain unless a YAML reader
    /// would take it for a null, a boolean or a number.
    Token(String),
    /// An integer field, always plain.
    Integer(u64),
    /// Free text: a literal block where it reads back exactly, double-quoted otherwise.
    Text(String),
}

impl Out {
    fn is_collection(&self) -> bool {
        matches!(self, Self::Mapping(_) | Self::Sequence(_))
    }
}

/// The file text for `value`: nothing for an empty top-level collection.
#[must_use]
pub fn write(value: &Out) -> String {
    let mut text = String::new();
    match value {
        Out::Mapping(fields) => mapping(&mut text, fields, 0, false),
        Out::Sequence(items) => sequence(&mut text, items, 0),
        scalar => {
            line_value(&mut text, scalar, 0);
        }
    }
    text
}

fn indent(text: &mut String, width: usize) {
    text.extend(std::iter::repeat_n(' ', width));
}

/// A mapping at `width`; one that is a sequence entry starts on the dash's line.
fn mapping(text: &mut String, fields: &[(&str, Out)], width: usize, after_dash: bool) {
    for (index, (key, value)) in fields.iter().enumerate() {
        if index > 0 || !after_dash {
            indent(text, width);
        }
        text.push_str(key);
        text.push(':');
        if value.is_collection() {
            text.push('\n');
            nested(text, value, width + 2);
        } else {
            text.push(' ');
            line_value(text, value, width);
        }
    }
}

fn sequence(text: &mut String, items: &[Out], width: usize) {
    for item in items {
        indent(text, width);
        text.push_str("- ");
        match item {
            Out::Mapping(fields) => mapping(text, fields, width + 2, true),
            Out::Sequence(_) => {
                text.pop();
                text.push('\n');
                nested(text, item, width + 2);
            }
            scalar => line_value(text, scalar, width),
        }
    }
}

fn nested(text: &mut String, value: &Out, width: usize) {
    match value {
        Out::Mapping(fields) => mapping(text, fields, width, false),
        Out::Sequence(items) => sequence(text, items, width),
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
        Out::Token(token) if is_plain(token) => text.push_str(token),
        Out::Token(token) => double_quoted(text, token),
        Out::Text(string) if is_literal(string) => literal(text, string, width + 2),
        Out::Text(string) => double_quoted(text, string),
        Out::Mapping(_) | Out::Sequence(_) => unreachable!("collections are not line values"),
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
    WORDS.contains(&token.to_ascii_lowercase().trim_start_matches('.'))
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

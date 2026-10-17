//! The value of a string literal that names a test: the text between its quotes, with each escape
//! read as its language reads it. A template literal with a substitution has no one value.

use std::ops::RangeInclusive;

use tree_sitter::Node;

use super::text_of;

/// The value of `node` when it is a string literal without a substitution: a JavaScript string or
/// template literal, or a Go interpreted or raw string.
pub(super) fn value(node: Node<'_>, text: &str) -> Option<String> {
    let literal = text_of(node, text);
    match node.kind() {
        "string" => Some(script(inner(literal)?)),
        "template_string" => {
            let mut cursor = node.walk();
            let substituted = node
                .children(&mut cursor)
                .any(|child| child.kind() == "template_substitution");
            (!substituted).then(|| script(inner(literal).unwrap_or_default()))
        }
        "interpreted_string_literal" => Some(go(inner(literal)?)),
        // Go leaves carriage returns out of a raw string.
        "raw_string_literal" => inner(literal).map(|raw| raw.replace('\r', "")),
        _ => None,
    }
}

/// The first argument of the call whose `arguments` node is given: its first named child that is
/// not a comment.
pub(super) fn first_argument(arguments: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = arguments.walk();
    arguments
        .named_children(&mut cursor)
        .find(|argument| !argument.kind().ends_with("comment"))
}

/// `literal` without its first and last character, its quotes.
fn inner(literal: &str) -> Option<&str> {
    let mut characters = literal.chars();
    characters.next()?;
    characters.next_back()?;
    Some(characters.as_str())
}

/// What a JavaScript escape is read as.
enum Read {
    Character(char),
    /// A line continuation, or a backslash that ends the string.
    Nothing,
    /// An escape that names no character, kept as written.
    AsWritten,
}

impl Read {
    fn of(code: Option<u32>) -> Self {
        code.and_then(char::from_u32)
            .map_or(Self::AsWritten, Self::Character)
    }
}

/// `text` with each escape read as JavaScript reads it: `\n`, `\t`, `\r`, `\b`, `\f` and `\v`;
/// `\xHH`, `\uXXXX`, two `\uXXXX` that form a surrogate pair, and `\u{…}` in hexadecimal; legacy
/// octal; a backslash before LF, CR, CR LF, U+2028 or U+2029 as nothing; and a backslash before any
/// other character as that character. An escape that names no character, such as a lone
/// surrogate, is kept as written.
fn script(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        let escape = &rest[at + 1..];
        let (read, length) = script_escape(escape);
        match read {
            Read::Character(character) => out.push(character),
            Read::Nothing => {}
            Read::AsWritten => out.push_str(&rest[at..=at + length]),
        }
        rest = &escape[length..];
    }
    out.push_str(rest);
    out
}

/// What the JavaScript escape at the start of `escape`, just after its backslash, is read as, and
/// how many bytes it takes.
fn script_escape(escape: &str) -> (Read, usize) {
    let Some(first) = escape.chars().next() else {
        return (Read::Nothing, 0);
    };
    let simple = match first {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        'b' => Some('\u{8}'),
        'f' => Some('\u{c}'),
        'v' => Some('\u{b}'),
        _ => None,
    };
    if let Some(character) = simple {
        return (Read::Character(character), 1);
    }
    match first {
        '\n' | '\u{2028}' | '\u{2029}' => (Read::Nothing, first.len_utf8()),
        '\r' => (
            Read::Nothing,
            if escape[1..].starts_with('\n') { 2 } else { 1 },
        ),
        'u' if escape[1..].starts_with('{') => {
            let (code, length) = braced(escape);
            (Read::of(code), length)
        }
        'u' => unit(escape),
        'x' => {
            let (code, length) = hexadecimal(escape, 2);
            (Read::of(code), length)
        }
        '0'..='7' => {
            // Legacy octal: up to three digits after `0` to `3`, up to two after `4` to `7`.
            let most = if first <= '3' { 3 } else { 2 };
            let (code, length) = octal(escape, 1..=most);
            (Read::of(code), length)
        }
        other => (Read::Character(other), other.len_utf8()),
    }
}

/// `\uXXXX`, one UTF-16 code unit: a character, or a high surrogate that the next `\uXXXX` pairs
/// with a low one into one character. A surrogate left alone is kept as written.
fn unit(escape: &str) -> (Read, usize) {
    let (Some(high), length) = hexadecimal(escape, 4) else {
        return (Read::AsWritten, 1);
    };
    if let Some(character) = char::from_u32(high) {
        return (Read::Character(character), length);
    }
    let low = escape[length..]
        .strip_prefix('\\')
        .filter(|next| next.starts_with('u'))
        .map(|next| hexadecimal(next, 4));
    match low {
        Some((Some(low), low_length))
            if (0xD800..=0xDBFF).contains(&high) && (0xDC00..=0xDFFF).contains(&low) =>
        {
            let code = 0x1_0000 + ((high - 0xD800) << 10) + (low - 0xDC00);
            (Read::of(Some(code)), length + 1 + low_length)
        }
        _ => (Read::AsWritten, length),
    }
}

/// What a Go escape is read as.
enum GoRead {
    Byte(u8),
    Character(char),
}

/// `text` with each escape read as Go reads it: `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, `\\`,
/// `\'` and `\"`; `\uXXXX` and `\UXXXXXXXX` as the character they name; and `\xHH` and three octal
/// digits as one byte. The bytes are read together as UTF-8, so `\xc3\xa9` is `é`. A string whose
/// bytes are not UTF-8, or that holds an escape Go does not define, is kept as written.
fn go(text: &str) -> String {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('\\') {
        bytes.extend_from_slice(&rest.as_bytes()[..at]);
        let escape = &rest[at + 1..];
        let Some((read, length)) = go_escape(escape) else {
            return text.to_owned();
        };
        match read {
            GoRead::Byte(byte) => bytes.push(byte),
            GoRead::Character(character) => {
                bytes.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
            }
        }
        rest = &escape[length..];
    }
    bytes.extend_from_slice(rest.as_bytes());
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_owned())
}

/// What the Go escape at the start of `escape`, just after its backslash, is read as, and how many
/// bytes it takes; none for an escape Go does not define.
fn go_escape(escape: &str) -> Option<(GoRead, usize)> {
    let first = escape.chars().next()?;
    let simple = match first {
        'a' => Some('\u{7}'),
        'b' => Some('\u{8}'),
        'f' => Some('\u{c}'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        'v' => Some('\u{b}'),
        '\\' | '\'' | '"' => Some(first),
        _ => None,
    };
    if let Some(character) = simple {
        return Some((GoRead::Character(character), 1));
    }
    let (read, length) = match first {
        'u' | 'U' => {
            let (code, length) = hexadecimal(escape, if first == 'u' { 4 } else { 8 });
            (GoRead::Character(char::from_u32(code?)?), length)
        }
        'x' => {
            let (code, length) = hexadecimal(escape, 2);
            (GoRead::Byte(u8::try_from(code?).ok()?), length)
        }
        '0'..='7' => {
            let (code, length) = octal(escape, 3..=3);
            (GoRead::Byte(u8::try_from(code?).ok()?), length)
        }
        _ => return None,
    };
    Some((read, length))
}

/// `\u{…}`: one to six hexadecimal digits in braces.
fn braced(escape: &str) -> (Option<u32>, usize) {
    let Some(close) = escape.find('}') else {
        return (None, 1);
    };
    let digits = &escape[2..close];
    let code = (1..=6)
        .contains(&digits.len())
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten();
    (code, close + 1)
}

/// A letter followed by exactly `count` hexadecimal digits: their value, and the bytes the
/// escape takes; no value, taking the letter alone, where the digits are not there.
fn hexadecimal(escape: &str, count: usize) -> (Option<u32>, usize) {
    let digits = escape.get(1..=count).filter(|digits| {
        digits.len() == count && digits.chars().all(|digit| digit.is_ascii_hexdigit())
    });
    match digits {
        Some(digits) => (u32::from_str_radix(digits, 16).ok(), 1 + count),
        None => (None, 1),
    }
}

/// As many octal digits from the start of `escape` as `counts` allows: their value, and the bytes
/// they take; no value where fewer are there than `counts` needs.
fn octal(escape: &str, counts: RangeInclusive<usize>) -> (Option<u32>, usize) {
    let length = escape
        .chars()
        .take(*counts.end())
        .take_while(|digit| ('0'..='7').contains(digit))
        .count();
    if !counts.contains(&length) {
        return (None, length.max(1));
    }
    (u32::from_str_radix(&escape[..length], 8).ok(), length)
}

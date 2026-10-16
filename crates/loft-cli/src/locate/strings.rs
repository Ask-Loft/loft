//! The value of a string literal that names a test: the text between its quotes, with each escape
//! read as the character it stands for. A template literal with a substitution has no one value.

use tree_sitter::Node;

use super::text_of;

/// The value of `node` when it is a string literal without a substitution.
pub(super) fn value(node: Node<'_>, text: &str) -> Option<String> {
    let literal = text_of(node, text);
    match node.kind() {
        "string" | "interpreted_string_literal" => Some(unescape(inner(literal)?)),
        "template_string" => {
            let mut cursor = node.walk();
            let substituted = node
                .children(&mut cursor)
                .any(|child| child.kind() == "template_substitution");
            (!substituted).then(|| unescape(inner(literal).unwrap_or_default()))
        }
        "raw_string_literal" => inner(literal).map(str::to_owned),
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

/// `text` with each escape read as the character it stands for: `\n`, `\t`, `\r`, `\b`, `\f`,
/// `\v` and `\a`; `\uXXXX`, `\u{…}`, `\UXXXXXXXX` and `\xHH` in hexadecimal; up to three octal
/// digits; a backslash before a line break as nothing; and a backslash before any other character
/// as that character. An escape that names no character is kept as written.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        let escape = &rest[at + 1..];
        let (read, length) = escaped(escape);
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

/// What an escape is read as.
enum Read {
    Character(char),
    /// A line continuation, or a backslash that ends the string.
    Nothing,
    /// An escape that names no character, kept as written.
    AsWritten,
}

impl Read {
    fn of(character: Option<char>) -> Self {
        character.map_or(Self::AsWritten, Self::Character)
    }
}

/// What the escape at the start of `escape`, just after its backslash, is read as, and how many
/// bytes it takes.
fn escaped(escape: &str) -> (Read, usize) {
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
        'a' => Some('\u{7}'),
        _ => None,
    };
    if let Some(character) = simple {
        return (Read::Character(character), 1);
    }
    let (character, length) = match first {
        '\n' => return (Read::Nothing, 1),
        'u' if escape[1..].starts_with('{') => braced(escape),
        'u' => hexadecimal(escape, 4),
        'U' => hexadecimal(escape, 8),
        'x' => hexadecimal(escape, 2),
        '0'..='7' => octal(escape),
        other => (Some(other), other.len_utf8()),
    };
    (Read::of(character), length)
}

/// `\u{…}`: up to six hexadecimal digits in braces.
fn braced(escape: &str) -> (Option<char>, usize) {
    let Some(close) = escape.find('}') else {
        return (None, 1);
    };
    let digits = &escape[2..close];
    let character = (1..=6)
        .contains(&digits.len())
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten()
        .and_then(char::from_u32);
    (character, close + 1)
}

/// A letter followed by exactly `count` hexadecimal digits.
fn hexadecimal(escape: &str, count: usize) -> (Option<char>, usize) {
    let digits = escape.get(1..=count).filter(|digits| {
        digits.len() == count && digits.chars().all(|digit| digit.is_ascii_hexdigit())
    });
    match digits {
        Some(digits) => {
            let character = u32::from_str_radix(digits, 16)
                .ok()
                .and_then(char::from_u32);
            (character, 1 + count)
        }
        None => (None, 1),
    }
}

/// Up to three octal digits.
fn octal(escape: &str) -> (Option<char>, usize) {
    let length = escape
        .chars()
        .take(3)
        .take_while(|digit| ('0'..='7').contains(digit))
        .count();
    let character = u32::from_str_radix(&escape[..length], 8)
        .ok()
        .and_then(char::from_u32);
    (character, length)
}

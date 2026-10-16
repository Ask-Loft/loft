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

/// `literal` without its first and last character, its quotes.
fn inner(literal: &str) -> Option<&str> {
    let mut characters = literal.chars();
    characters.next()?;
    characters.next_back()?;
    Some(characters.as_str())
}

/// `text` with each escape read: `\n`, `\t`, `\r` and `\0` as those characters, a backslash before
/// a line break as nothing, and a backslash before any other character as that character.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\n') | None => {}
            Some(other) => out.push(other),
        }
    }
    out
}

//! Text that a remote, its authorization server or the tree gave, as an error or a warning writes
//! it: each control character and each bidirectional control escaped, so that every character of
//! it reaches the terminal as visible text, and none as a command to the terminal.

use std::fmt::{self, Write as _};

use serde_json::Value;

/// The bidirectional controls, Unicode's property `Bidi_Control`.
const BIDI_CONTROLS: [char; 12] = [
    '\u{61c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

/// `text` as an error writes it: each control character, of Unicode's general category `Cc`, and
/// each bidirectional control, of Unicode's property `Bidi_Control`, as `\u{<hex>}`, its code
/// point in lowercase hexadecimal without leading zeros; each backslash as `\\`, so that an
/// escaped text stands for one text alone; and every other character as it is.
#[derive(Debug, Clone, Copy)]
pub struct Escaped<'a>(pub &'a str);

impl fmt::Display for Escaped<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for character in self.0.chars() {
            if character == '\\' {
                formatter.write_str("\\\\")?;
            } else if character.is_control() || BIDI_CONTROLS.contains(&character) {
                write!(formatter, "{}", character.escape_unicode())?;
            } else {
                formatter.write_char(character)?;
            }
        }
        Ok(())
    }
}

/// `value`, a JSON value that a remote gave, as an error or a warning names it: a string between
/// double quotes, its text [`Escaped`]; any other value as JSON writes it, [`Escaped`] as a
/// whole, since JSON leaves some control characters as they are.
#[must_use]
pub fn value(value: &Value) -> String {
    match value {
        Value::String(text) => format!("\"{}\"", Escaped(text)),
        other => Escaped(&other.to_string()).to_string(),
    }
}

/// Text that holds control characters of both ranges, a bidirectional control and a backslash,
/// for the tests of what names text a remote gave.
#[cfg(test)]
pub const HOSTILE: &str = "x\u{1b}]0;y\u{7}\u{9b}2J\u{202e}\\z";

/// [`HOSTILE`] as [`Escaped`] writes it.
#[cfg(test)]
pub const ESCAPED: &str = "x\\u{1b}]0;y\\u{7}\\u{9b}2J\\u{202e}\\\\z";

/// Whether `text` holds no control character, of Unicode's general category `Cc`, and no
/// bidirectional control, of Unicode's property `Bidi_Control`, so that a terminal shows each of
/// its characters.
#[cfg(test)]
pub fn shown(text: &str) -> bool {
    !text
        .chars()
        .any(|character| character.is_control() || BIDI_CONTROLS.contains(&character))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ESCAPED, Escaped, HOSTILE, shown, value};

    fn escaped(text: &str) -> String {
        Escaped(text).to_string()
    }

    #[test]
    fn each_control_character_is_written_as_its_code_point_in_lowercase_hexadecimal() {
        // Unicode's general category `Cc`: U+0000 to U+001F and U+007F to U+009F.
        for code in (0..=0x1f).chain(0x7f..=0x9f) {
            let character = char::from_u32(code).unwrap();
            let text = format!("a{character}b");
            assert_eq!(escaped(&text), format!("a\\u{{{code:x}}}b"), "{code:#x}");
        }
        assert_eq!(escaped("\u{1b}[0m"), "\\u{1b}[0m");
        assert_eq!(escaped("\0\t\n"), "\\u{0}\\u{9}\\u{a}");
    }

    #[test]
    fn each_bidirectional_control_is_written_as_its_code_point() {
        // Unicode's property `Bidi_Control`.
        for code in [
            0x61c, 0x200e, 0x200f, 0x202a, 0x202b, 0x202c, 0x202d, 0x202e, 0x2066, 0x2067, 0x2068,
            0x2069,
        ] {
            let character = char::from_u32(code).unwrap();
            assert_eq!(escaped(&character.to_string()), format!("\\u{{{code:x}}}"));
        }
        assert_eq!(escaped("\u{202e}"), "\\u{202e}");
    }

    #[test]
    fn a_backslash_is_doubled_so_that_an_escaped_text_stands_for_one_text_alone() {
        assert_eq!(escaped("a\\b"), "a\\\\b");
        assert_eq!(escaped("\\u{1b}"), "\\\\u{1b}");
        assert_ne!(escaped("\\u{1b}"), escaped("\u{1b}"));
    }

    #[test]
    fn every_other_character_is_written_as_it_is() {
        for text in [
            "",
            "https://remote.example/documents/one?a=b#c",
            "`code` \"quoted\" 'single' ~!@$%^&*()",
            "Bücher, 日本語, Ελληνικά, a non-breaking\u{a0}space",
            "a zero-width\u{200b}space and a soft\u{ad}hyphen",
        ] {
            assert_eq!(escaped(text), text);
        }
    }

    #[test]
    fn a_json_value_is_written_escaped_a_string_between_double_quotes() {
        assert_eq!(Escaped(HOSTILE).to_string(), ESCAPED);
        assert_eq!(value(&json!(HOSTILE)), format!("\"{ESCAPED}\""));
        assert_eq!(value(&json!("a\"b")), "\"a\"b\"");
        assert_eq!(value(&json!("")), "\"\"");
        assert_eq!(value(&json!(3)), "3");
        assert_eq!(value(&json!(null)), "null");
        // Any other value is written as JSON, which escapes some control characters its own way,
        // and then escaped as a whole.
        let written = value(&json!(["\u{1b}\u{9b}"]));
        assert_eq!(written, r#"["\\u001b\u{9b}"]"#);
        for shown_value in [value(&json!(HOSTILE)), written] {
            assert!(shown(&shown_value), "{shown_value:?}");
        }
    }
}

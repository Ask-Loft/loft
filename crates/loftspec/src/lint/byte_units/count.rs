//! A count of bytes that gives a decimal byte unit: a number or a power equal to the unit's
//! value, 1000^k or 1024^k bytes for its power k, then `byte` or `bytes`, closing its clause.

use crate::text::strip_prefix_ignoring_case;

/// The marks that may follow a count of bytes that closes its clause, beside the end of the
/// text; a line break ends a paragraph, a list item or a heading in the text shown.
const CLOSING: [char; 5] = ['.', ',', ';', ')', '\n'];

/// Whether `text` opens on a count of bytes equal to the value of a unit of power `k`, such as
/// `1,000,000 bytes` or `2^20 bytes` for `MB`, with nothing after it but one of `CLOSING` or the
/// end of the text.
pub(super) fn gives_value(text: &str, k: u32) -> bool {
    let Some((value, rest)) = power(text).or_else(|| number(text)) else {
        return false;
    };
    let rest = rest.trim_start();
    let word = strip_prefix_ignoring_case(rest, "bytes")
        .or_else(|| strip_prefix_ignoring_case(rest, "byte"));
    let closes = word.is_some_and(|after| after.is_empty() || after.starts_with(CLOSING));
    closes
        && [1000_u128, 1024]
            .iter()
            .any(|base| base.checked_pow(k) == Some(value))
}

/// Whether a count of bytes equal to the value of a unit of power `k` stands anywhere in
/// `text`, each count starting a word.
pub(super) fn holds_value(text: &str, k: u32) -> bool {
    text.char_indices().any(|(at, c)| {
        let before = text[..at].chars().next_back();
        let starts =
            !before.is_some_and(|b| b.is_alphanumeric() || matches!(b, '_' | ',' | '.' | '^'));
        c.is_ascii_digit() && starts && gives_value(&text[at..], k)
    })
}

/// A power of any integer base, such as `10^6`, `1000^2` or `2 ^ 20`, at the start of `text`:
/// its value and the text after it.
fn power(text: &str) -> Option<(u128, &str)> {
    let (base, rest) = digits(text)?;
    let rest = rest.trim_start().strip_prefix('^')?.trim_start();
    let (exponent, rest) = digits(rest)?;
    Some((base.checked_pow(u32::try_from(exponent).ok()?)?, rest))
}

/// The run of digits that opens `text`, where one does: its value and the text after it.
fn digits(text: &str) -> Option<(u128, &str)> {
    let rest = text.trim_start_matches(|c: char| c.is_ascii_digit());
    let run = &text[..text.len() - rest.len()];
    Some((run.parse().ok()?, rest))
}

/// A number at the start of `text`: digits, with commas, full stops and spaces between them,
/// read as one integer. Its value and the text after its last digit.
fn number(text: &str) -> Option<(u128, &str)> {
    if !text.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let (mut value, mut last_digit) = (0_u128, 0);
    for (at, c) in text.char_indices() {
        if let Some(digit) = c.to_digit(10) {
            value = value.checked_mul(10)?.checked_add(u128::from(digit))?;
            last_digit = at + 1;
        } else if !matches!(c, ',' | '.' | ' ' | '\u{a0}' | '\u{202f}') {
            break;
        }
    }
    Some((value, &text[last_digit..]))
}

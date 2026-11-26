//! A count of bytes that gives a decimal byte unit: digits, comma groups of three or a power
//! equal to the unit's value, 1000^k or 1024^k bytes for its power k, then `byte` or `bytes`,
//! closing its clause.

use crate::lint::text::is_written_number;
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

/// A number written with digits at the start of `text`, as `LQ-002` reads one: digits, or
/// digits grouped by commas in threes, such as `1,048,576`. Its value and the text after it.
fn number(text: &str) -> Option<(u128, &str)> {
    let run = text.trim_start_matches(|c: char| c.is_ascii_digit() || c == ',');
    let written = text[..text.len() - run.len()].trim_end_matches(',');
    if !is_written_number(written) {
        return None;
    }
    let digits: String = written.chars().filter(char::is_ascii_digit).collect();
    Some((digits.parse().ok()?, &text[written.len()..]))
}

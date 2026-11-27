//! A count of bytes that gives a decimal byte unit: digits, comma groups of three or a power
//! equal to the unit's value, 1000^k or 1024^k bytes for its power k, then `byte` or `bytes`,
//! ending its sentence.

use super::is_space;
use crate::lint::text::is_written_number;
use crate::text::strip_prefix_ignoring_case;

/// The marks that may follow a count of bytes that ends its sentence, beside the end of the
/// text; a line break ends a paragraph, a list item or a heading in the text shown. A `,` or a
/// `;` may lead on to another value, as in `1,000,000 bytes, or 1,048,576 bytes`.
const CLOSING: [char; 3] = ['.', ')', '\n'];

/// The base of a decimal unit's value: 1000^k bytes, or 1024^k bytes, for its power k.
#[derive(Clone, Copy)]
pub(super) enum Base {
    Thousand,
    Kibi,
}

/// The base of the value of a unit of power `k` that `text` opens on as a count of bytes, such as
/// `1,000,000 bytes` or `2^20 bytes` for `MB`, with nothing after it but one of `CLOSING` or the
/// end of the text; none where `text` opens on no such count.
pub(super) fn given_base(text: &str, k: u32) -> Option<Base> {
    let (value, rest) = power(text).or_else(|| number(text))?;
    let rest = rest.trim_start_matches(is_space);
    let word = strip_prefix_ignoring_case(rest, "bytes")
        .or_else(|| strip_prefix_ignoring_case(rest, "byte"))?;
    if !(word.is_empty() || word.starts_with(CLOSING)) {
        return None;
    }
    [(1000_u128, Base::Thousand), (1024, Base::Kibi)]
        .into_iter()
        .find(|(base, _)| base.checked_pow(k) == Some(value))
        .map(|(_, base)| base)
}

/// A power of any integer base, such as `10^6`, `1000^2` or `2 ^ 20`, at the start of `text`:
/// its value and the text after it.
fn power(text: &str) -> Option<(u128, &str)> {
    let (base, rest) = digits(text)?;
    let rest = rest.trim_start_matches(is_space).strip_prefix('^')?;
    let rest = rest.trim_start_matches(is_space);
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

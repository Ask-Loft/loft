//! `V2-014` (F-21): a quantity stated in a decimal byte unit, such as `512 MB`, in a document
//! that also states one in a binary byte unit, such as `4 KiB`, and never says what its decimal
//! units mean. A reader then cannot tell whether `1 MB` is 1,000,000 bytes or 1,048,576.
//!
//! Every node and every decision is read, whatever the decision's state, as the text a Markdown
//! reader shows, so that emphasis drops its marks and code spans and fenced code state nothing.
//! A quantity is a number followed by a unit, with nothing, one space, one no-break space or one
//! hyphen between them; units are matched in their own case, so `Mb` is neither family. A number
//! inside a web address, or right after a letter, `_`, `/`, `.` or `-`, begins no quantity. One
//! definition of a decimal unit settles every decimal unit of the document: a term whose name or
//! alias is one, or a text that gives one, by its symbol or its full name, as a count of bytes
//! (`1 MB = 1,000,000 bytes`, `MB: 10^6 bytes`, `A megabyte is 1000^2 bytes`). Since one false
//! definition silences the whole document, a text defines a unit only where the unit opens a
//! clause, alone or after `1`, `one`, `a`, `an` or `the`.

use super::context::{Context, Item};
use super::text::plain;
use super::{Raw, RuleId};
use crate::identity::Key;
use crate::node::Node;

const DECIMAL: [&str; 5] = ["kB", "KB", "MB", "GB", "TB"];
const BINARY: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
/// The full names of the decimal units, as a term may be named, in either number.
const DECIMAL_NAMES: [&str; 4] = ["kilobyte", "megabyte", "gigabyte", "terabyte"];
/// The words after a unit that make the text after them its definition.
const DEFINING: [&str; 7] = ["=", ":", "is", "means", "equals", "denotes", "stands for"];
/// The words that may stand between the opening of a clause and the unit it defines.
const LEADERS: [&str; 5] = ["1", "one", "a", "an", "the"];
/// The marks after which a clause opens, beside the start of a text or a line.
const CLAUSE_MARKS: [char; 8] = ['\n', '.', '!', '?', ':', ';', ',', '('];

/// What one element's texts say about byte units.
#[derive(Default)]
struct Use {
    /// The first decimal unit a quantity is stated in, where one is.
    decimal: Option<&'static str>,
    binary: bool,
    defines: bool,
}

impl Use {
    fn read(&mut self, text: &str) {
        // Every unit ends in `B`, and every full name holds a `b`.
        if !text.contains(['B', 'b']) {
            return;
        }
        let shown = plain(text);
        let units = units(&shown);
        for &(at, unit) in &units {
            if is_quantity(&shown, at) {
                self.note(unit);
            }
        }
        self.defines |= defines_decimal(&shown, &units);
    }

    /// A Quality Attribute's unit field: a byte unit there is a quantity in it.
    fn unit(&mut self, unit: &str) {
        let unit = unit.trim();
        if let Some(unit) = DECIMAL.iter().chain(&BINARY).find(|each| **each == unit) {
            self.note(unit);
        }
    }

    /// A quantity in `unit`: binary, or the first decimal one.
    fn note(&mut self, unit: &'static str) {
        if BINARY.contains(&unit) {
            self.binary = true;
        } else {
            self.decimal.get_or_insert(unit);
        }
    }

    const fn states_a_unit(&self) -> bool {
        self.binary || self.decimal.is_some()
    }
}

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    let document = context.document;
    let termed = document.terms.iter().any(|term| {
        let definition = plain(&term.definition);
        let mut names = std::iter::once(&term.name).chain(&term.aliases);
        names.any(|name| names_decimal_unit(name))
            || defines_decimal(&definition, &units(&definition))
    });
    if termed {
        return;
    }
    let nodes = (context.items.iter()).map(|item| (item.id.key(), node_use(item)));
    let decisions = (document.decisions.iter()).map(|decision| {
        let mut each = Use::default();
        for text in decision.texts() {
            each.read(text);
        }
        (decision.id.key(), each)
    });
    let mut defined = false;
    let uses: Vec<(Key, Use)> = nodes
        .chain(decisions)
        .inspect(|(_, each)| defined |= each.defines)
        .filter(|(_, each)| each.states_a_unit())
        .collect();
    if defined || !uses.iter().any(|(_, each)| each.binary) {
        return;
    }
    for (key, each) in &uses {
        if let Some(unit) = each.decimal {
            // Another element where there is one, so that the finding points beyond itself.
            let binary = (uses.iter())
                .find(|(other, them)| them.binary && other != key)
                .map_or(*key, |(other, _)| *other);
            let message = format!(
                "A quantity in a decimal byte unit stands beside quantities in binary byte \
                 units, as in {binary}, and the document defines no decimal byte unit."
            );
            out.push(Raw::new(RuleId::V2014, vec![*key], message).term(unit));
        }
    }
}

/// What a node's name or statement, body, target and required value say.
fn node_use(item: &Item<'_>) -> Use {
    let mut each = Use::default();
    each.read(&item.name());
    if let Some(body) = item.body() {
        each.read(body);
    }
    match item.node {
        Node::QualityAttribute(quality) => {
            let target = &quality.target;
            each.unit(&target.unit);
            for text in [&target.metric, &target.scenario, &target.environment] {
                each.read(text);
            }
        }
        Node::Constraint(constraint) => {
            if let Some(binding) = &constraint.binding {
                each.read(&binding.required_value);
            }
        }
        _ => {}
    }
    each
}

/// Whether a term's name is a decimal unit, or its full name, in either number: a lowercase `s`
/// makes it plural, so that `MBs` is a unit and `KBS` an acronym.
fn names_decimal_unit(name: &str) -> bool {
    let name = name.trim();
    let singular = name.strip_suffix('s').unwrap_or(name);
    DECIMAL.contains(&singular)
        || DECIMAL_NAMES
            .iter()
            .any(|full| singular.eq_ignore_ascii_case(full))
}

/// Each unit in `text` that stands as a word of its own, with its byte offset. The text is
/// scanned by hand, not by `crate::text::token_ranges`, since `512MB` is one token there.
fn units(text: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for unit in DECIMAL.iter().chain(&BINARY) {
        for (at, _) in text.match_indices(unit) {
            let after = text[at + unit.len()..].chars().next();
            let before = text[..at].chars().next_back();
            if !after.is_some_and(is_word) && !before.is_some_and(char::is_alphabetic) {
                found.push((at, *unit));
            }
        }
    }
    found.sort_unstable();
    found
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the unit at `at` follows a number, with nothing, one space, one no-break space (wide
/// or narrow, as `number` reads inside a number) or one hyphen between them, where the number
/// starts a word outside a web address.
fn is_quantity(text: &str, at: usize) -> bool {
    let head = &text[..at];
    let head = head
        .strip_suffix([' ', '\u{a0}', '\u{202f}', '-'])
        .unwrap_or(head);
    let run = head.trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, ',' | '.'));
    let number = head[run.len()..].trim_start_matches([',', '.']);
    let before = &head[..head.len() - number.len()];
    number.ends_with(|c: char| c.is_ascii_digit())
        && !before.ends_with(|c: char| is_word(c) || matches!(c, '/' | '.' | '-'))
        && !in_web_address(text, before.len())
}

/// Whether the word around `at` is a web address.
fn in_web_address(text: &str, at: usize) -> bool {
    let start = (text[..at].char_indices().rev())
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(space, c)| space + c.len_utf8());
    let end = (text[at..].find(char::is_whitespace)).map_or(text.len(), |space| at + space);
    let word = &text[start..end];
    word.contains("://") || strip_prefix_ignoring_case(word, "www.").is_some()
}

/// Whether `text`, whose units are `units`, gives a decimal unit, by its symbol or its full
/// name, as a count of bytes, where the unit opens a clause.
fn defines_decimal(text: &str, units: &[(usize, &'static str)]) -> bool {
    let symbols = (units.iter())
        .filter(|(_, unit)| DECIMAL.contains(unit))
        .map(|&(at, unit)| (at, at + unit.len()));
    let mut spans = symbols.chain(full_names(text));
    spans.any(|(start, end)| {
        opens_clause(&text[..start]) && definition_follows(text[end..].trim_start())
    })
}

/// Whether a unit after `head` opens a clause, alone or after one of `LEADERS`.
fn opens_clause(head: &str) -> bool {
    let head = head.trim_end_matches([' ', '\t', '\u{a0}']);
    let head = (LEADERS.iter())
        .find_map(|leader| {
            let cut = head.len().checked_sub(leader.len())?;
            let (rest, word) = (head.get(..cut)?, &head[cut..]);
            let bounded = !rest.ends_with(|c: char| is_word(c) || matches!(c, '.' | ','));
            (word.eq_ignore_ascii_case(leader) && bounded).then_some(rest)
        })
        .unwrap_or(head);
    let head = head.trim_end_matches([' ', '\t', '\u{a0}']);
    head.is_empty() || head.ends_with(CLAUSE_MARKS)
}

/// Where each full name of a decimal unit in `text`, in either number and any case, starts and
/// ends.
fn full_names(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for (at, _) in text.char_indices() {
        for name in DECIMAL_NAMES {
            let Some(after) = strip_prefix_ignoring_case(&text[at..], name) else {
                continue;
            };
            let after = strip_prefix_ignoring_case(after, "s").unwrap_or(after);
            let end = text.len() - after.len();
            let before = text[..at].chars().next_back();
            if !before.is_some_and(is_word) && !after.starts_with(is_word) {
                spans.push((at, end));
            }
        }
    }
    spans
}

/// Whether `rest`, the text after a unit, defines it: a defining word and a count of bytes, or a
/// parenthesis that opens on a count of bytes.
fn definition_follows(rest: &str) -> bool {
    if let Some(inside) = rest.strip_prefix('(') {
        return byte_count(inside.trim_start());
    }
    DEFINING.iter().any(|word| {
        strip_prefix_ignoring_case(rest, word).is_some_and(|after| {
            let boundary = matches!(*word, "=" | ":") || after.starts_with(char::is_whitespace);
            let after = after.trim_start();
            let after = strip_prefix_ignoring_case(after, "exactly").map_or(after, str::trim_start);
            boundary && byte_count(after)
        })
    })
}

/// Whether `text` opens on a count of bytes: a number, or a power such as `10^6` or `1000^2`,
/// then `byte` or `bytes` as a word.
fn byte_count(text: &str) -> bool {
    let rest = power(text).or_else(|| number(text));
    rest.is_some_and(|rest| {
        let rest = rest.trim_start();
        let word = strip_prefix_ignoring_case(rest, "bytes")
            .or_else(|| strip_prefix_ignoring_case(rest, "byte"));
        word.is_some_and(|after| !after.starts_with(is_word))
    })
}

/// `text` after `prefix`, where `text` opens on it in any ASCII case.
fn strip_prefix_ignoring_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// The text after a power of any integer base, such as `10^6`, `1000^2` or `2 ^ 20`, at the
/// start of `text`.
fn power(text: &str) -> Option<&str> {
    let rest = digits(text)?.trim_start().strip_prefix('^')?.trim_start();
    digits(rest)
}

/// The text after the run of digits that opens `text`, where one does.
fn digits(text: &str) -> Option<&str> {
    let rest = text.trim_start_matches(|c: char| c.is_ascii_digit());
    (rest.len() < text.len()).then_some(rest)
}

/// The text after a number at the start of `text`: digits, with commas, full stops and spaces
/// between them.
fn number(text: &str) -> Option<&str> {
    if !text.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let mut last_digit = 0;
    for (at, c) in text.char_indices() {
        if c.is_ascii_digit() {
            last_digit = at + 1;
        } else if !matches!(c, ',' | '.' | ' ' | '\u{a0}' | '\u{202f}') {
            break;
        }
    }
    Some(&text[last_digit..])
}

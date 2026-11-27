//! `V2-014` (F-21): a quantity stated in a decimal byte unit, such as `512 MB`, in a document
//! that also states one in a binary byte unit, such as `4 KiB`, and never says what its decimal
//! units mean. A reader then cannot tell whether `1 MB` is 1,000,000 bytes or 1,048,576.
//!
//! Every node, every decision, whatever its state, and every term's definition is read as the
//! text a Markdown reader shows, so that emphasis drops its marks, code states nothing and each
//! paragraph, list item and heading stands apart. A quantity is a number followed by a unit, with
//! nothing, one space, one no-break space or one hyphen between them; units are matched in their
//! own case, so `Mb` is neither family. A number inside a web address, or right after a letter or
//! one of `_ / . - = ? & #`, begins no quantity, except a range such as `4-16 MB`, where the
//! number before the `-` ends in a digit and comes after none of those, and a decimal such as
//! `.5 MB`, where the `.` comes after none of those.
//!
//! A definition of a decimal unit settles every decimal unit of its power, `kB` and `KB` alike,
//! and no other. Since a false definition silences every finding for its units, a definition is
//! narrow: a count of bytes equal to the unit's value, 1000^k or 1024^k, that ends its sentence,
//! given in parentheses right after the unit where no number but `1` comes before it
//! (`MB (10^6 bytes)`), or after a defining word where the unit opens a clause
//! (`1 MB = 1,000,000 bytes`, `A megabyte is 2^20 bytes`); or a term named after the unit whose
//! definition opens on its value, or on its full name, `of` and its value
//! (`A kilobyte of 1,000 bytes.`), after at most one leader or `exactly`. A decision defines a
//! unit only where it is accepted, and never in an option it did not select. A count or a
//! definition never runs on past the end of its paragraph, list item or heading.

use super::context::{Context, Item};
use super::text::plain;
use super::{Raw, RuleId};
use crate::document::TermEntry;
use crate::identity::Key;
use crate::node::Node;
use crate::text::strip_prefix_ignoring_case;

mod count;

const DECIMAL: [&str; 5] = ["kB", "KB", "MB", "GB", "TB"];
const BINARY: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
/// The full names of the decimal units, as a term may be named, in either number.
const DECIMAL_NAMES: [&str; 4] = ["kilobyte", "megabyte", "gigabyte", "terabyte"];
/// The words after a unit that make the text after them its definition.
const DEFINING: [&str; 7] = ["=", ":", "is", "means", "equals", "denotes", "stands for"];
/// The words that may stand between the opening of a clause and the unit it defines.
const LEADERS: [&str; 5] = ["1", "one", "a", "an", "the"];
/// The marks after which a clause opens, beside the start of the text; a line break starts a
/// paragraph, a list item or a heading in the text shown.
const CLAUSE_MARKS: [char; 8] = ['\n', '.', '!', '?', ':', ';', ',', '('];

/// The powers of every decimal unit, as bits of `Use::defines`: all of them defined.
const EVERY_POWER: u8 = 0b1_1110;

/// What one element's texts say about byte units.
#[derive(Default)]
struct Use {
    /// The decimal units quantities are stated in, each once, in the order first stated.
    decimal: Vec<&'static str>,
    binary: bool,
    /// The powers of the decimal units the element defines, bit k for power k.
    defines: u8,
}

impl Use {
    /// Reads `text` for quantities and, where it is in force, for definitions.
    fn read(&mut self, text: &str, in_force: bool) {
        // Every unit ends in `B`, and a full name holds `byte` or, in capitals, a `B`; most
        // prose holds neither, and is not given a Markdown parse.
        if !text.contains('B') && !text.contains("byte") {
            return;
        }
        let shown = plain(text);
        let units = units(&shown);
        for &(at, unit) in &units {
            if is_quantity(&shown, at) {
                self.note(unit);
            }
        }
        if in_force {
            self.defines |= defined_powers(&shown, &units);
        }
    }

    /// A Quality Attribute's unit field: a byte unit there is a quantity in it.
    fn unit(&mut self, unit: &str) {
        let unit = unit.trim();
        if let Some(unit) = DECIMAL.iter().chain(&BINARY).find(|each| **each == unit) {
            self.note(unit);
        }
    }

    /// A quantity in `unit`, binary or decimal.
    fn note(&mut self, unit: &'static str) {
        if BINARY.contains(&unit) {
            self.binary = true;
        } else if !self.decimal.contains(&unit) {
            self.decimal.push(unit);
        }
    }

    fn states_a_unit(&self) -> bool {
        self.binary || !self.decimal.is_empty()
    }
}

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    let document = context.document;
    let terms = (document.terms.iter()).map(|term| {
        let mut each = Use::default();
        each.read(&term.definition, true);
        each.defines |= term_powers(term);
        (term.id.key(), each)
    });
    let nodes = (context.items.iter()).map(|item| (item.id.key(), node_use(item)));
    let decisions = (document.decisions.iter()).map(|decision| {
        let mut each = Use::default();
        for text in decision.texts() {
            each.read(text, false);
        }
        for text in decision.texts_in_force() {
            each.read(text, true);
        }
        (decision.id.key(), each)
    });
    let mut uses: Vec<(Key, Use)> = Vec::new();
    let mut defined = 0;
    // The elements are read lazily, so that defining every power ends the reading.
    for (key, each) in terms.chain(nodes).chain(decisions) {
        defined |= each.defines;
        if defined == EVERY_POWER {
            return;
        }
        if each.states_a_unit() {
            uses.push((key, each));
        }
    }
    if !uses.iter().any(|(_, each)| each.binary) {
        return;
    }
    for (key, each) in &uses {
        let undefined = (each.decimal.iter()).find(|unit| defined & bit(symbol_power(unit)) == 0);
        if let Some(&unit) = undefined {
            // Another element where there is one, so that the finding points beyond itself.
            let binary = (uses.iter())
                .find(|(other, them)| them.binary && other != key)
                .map_or(*key, |(other, _)| *other);
            let message = format!(
                "A quantity in a decimal byte unit stands beside quantities in binary byte \
                 units, as in {binary}, and the document does not define {unit}."
            );
            out.push(Raw::new(RuleId::V2014, vec![*key], message).term(unit));
        }
    }
}

/// What a node's name or statement, body, target and required value say.
fn node_use(item: &Item<'_>) -> Use {
    let mut each = Use::default();
    each.read(&item.name(), true);
    if let Some(body) = item.body() {
        each.read(body, true);
    }
    match item.node {
        Node::QualityAttribute(quality) => {
            let target = &quality.target;
            each.unit(&target.unit);
            for text in [&target.metric, &target.scenario, &target.environment] {
                each.read(text, true);
            }
        }
        Node::Constraint(constraint) => {
            if let Some(binding) = &constraint.binding {
                each.read(&binding.required_value, true);
            }
        }
        _ => {}
    }
    each
}

/// The powers of the decimal units `term` is named after, by its name or an alias, that its
/// definition gives as a count of bytes: one opening the definition, or after the unit's full
/// name and `of`, after at most one of `LEADERS` or `exactly`.
fn term_powers(term: &TermEntry) -> u8 {
    let names = std::iter::once(&term.name).chain(&term.aliases);
    let powers: Vec<u32> = names.filter_map(|name| named_power(name)).collect();
    if powers.is_empty() {
        return 0;
    }
    let shown = plain(&term.definition);
    let opening = shown.trim_start();
    let opening = (LEADERS.iter().chain(&["exactly"]))
        .find_map(|leader| {
            let after = strip_prefix_ignoring_case(opening, leader)?;
            after
                .starts_with(is_space)
                .then(|| after.trim_start_matches(is_space))
        })
        .unwrap_or(opening);
    let given = |k: u32| count::gives_value(opening, k) || of_value(opening, k);
    (powers.into_iter().filter(|&k| given(k))).fold(0, |defined, k| defined | bit(k))
}

/// Whether `opening` opens on the full name of a unit of power `k`, then `of` and its value as a
/// count of bytes, as in `kilobyte of 1,000 bytes`.
fn of_value(opening: &str, k: u32) -> bool {
    (full_names(opening).into_iter())
        .filter(|&(start, _, power)| start == 0 && power == k)
        .any(|(_, end, _)| {
            let rest = &opening[end..];
            let after = strip_prefix_ignoring_case(rest.trim_start_matches(is_space), "of");
            rest.starts_with(is_space)
                && after.is_some_and(|after| {
                    after.starts_with(is_space)
                        && count::gives_value(after.trim_start_matches(is_space), k)
                })
        })
}

/// The bit of `Use::defines` that stands for power `k`.
const fn bit(k: u32) -> u8 {
    1 << k
}

/// The power of the decimal unit a term's name is, by its symbol or its full name, in either
/// number: a lowercase `s` makes it plural, so that `MBs` is a unit and `KBS` an acronym.
fn named_power(name: &str) -> Option<u32> {
    let name = name.trim();
    let singular = name.strip_suffix('s').unwrap_or(name);
    let by_name = (DECIMAL_NAMES.iter()).position(|full| singular.eq_ignore_ascii_case(full));
    let k = by_name.map(|at| at + 1);
    DECIMAL
        .contains(&singular)
        .then(|| symbol_power(singular))
        .or_else(|| k.and_then(|k| u32::try_from(k).ok()))
}

/// The power k of a decimal unit's symbol: its value is 1000^k or 1024^k bytes.
fn symbol_power(unit: &str) -> u32 {
    match unit {
        "MB" => 2,
        "GB" => 3,
        "TB" => 4,
        _ => 1,
    }
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

/// Whether `c` is a space within a line: any white space but a line break or separator (line
/// feed to carriage return, U+0085, U+2028, U+2029). The text shown ends each paragraph, list
/// item and heading with a line feed, and a separator ends a line as well.
fn is_space(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\n'..='\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the unit at `at` follows a number, with nothing, one space, one no-break space (wide
/// or narrow) or one hyphen between them, where the number lies outside a web address and comes
/// right after none of a letter, a digit or one of `_ / . - = ? & #`. A range such as `4-16 MB`
/// begins a quantity all the same where the number before the `-` ends in a digit and comes
/// after none of those, and a decimal such as `.5 MB` where the `.` comes after none of those.
fn is_quantity(text: &str, at: usize) -> bool {
    let head = &text[..at];
    let head = head
        .strip_suffix([' ', '\u{a0}', '\u{202f}', '-'])
        .unwrap_or(head);
    let (before, number) = split_number(head);
    let range = before.strip_suffix('-').is_some_and(|rest| {
        let (earlier, first) = split_number(rest);
        first.ends_with(|c: char| c.is_ascii_digit()) && !blocked(earlier)
    });
    let decimal = before.strip_suffix('.').is_some_and(|rest| !blocked(rest));
    number.ends_with(|c: char| c.is_ascii_digit())
        && (range || decimal || !blocked(before))
        && !in_web_address(text, before.len())
}

/// `head` cut before the number that ends it: digits with commas and full stops between them,
/// starting on a digit. The number is empty where `head` ends on none.
fn split_number(head: &str) -> (&str, &str) {
    let run = head.trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, ',' | '.'));
    let number = head[run.len()..].trim_start_matches([',', '.']);
    (&head[..head.len() - number.len()], number)
}

/// Whether a number right after `before` begins no quantity.
fn blocked(before: &str) -> bool {
    before.ends_with(|c: char| is_word(c) || "/.-=?&#".contains(c))
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

/// The powers of the decimal units that `text`, whose units are `units`, gives by symbol or by
/// full name as a count of bytes: in parentheses right after the unit, where no number but `1`
/// comes before it, or after a defining word where the unit opens a clause.
fn defined_powers(text: &str, units: &[(usize, &'static str)]) -> u8 {
    let symbols = (units.iter())
        .filter(|(_, unit)| DECIMAL.contains(unit))
        .map(|&(at, unit)| (at, at + unit.len(), symbol_power(unit)));
    let spans = symbols.chain(full_names(text));
    let given = spans.filter(|&(start, end, k)| {
        let rest = text[end..].trim_start_matches(is_space);
        match rest.strip_prefix('(') {
            Some(inside) => {
                unmultiplied(&text[..start])
                    && count::gives_value(inside.trim_start_matches(is_space), k)
            }
            None => opens_clause(&text[..start]) && defined_after_word(rest, k),
        }
    });
    given.fold(0, |defined, (.., k)| defined | bit(k))
}

/// Whether no number but `1` comes right before a unit after `head`, with nothing, one space,
/// one no-break space or one hyphen between them, so that `2 MB (2^20 bytes)` defines nothing.
fn unmultiplied(head: &str) -> bool {
    let head = head
        .strip_suffix([' ', '\u{a0}', '\u{202f}', '-'])
        .unwrap_or(head);
    let (_, number) = split_number(head);
    number.is_empty() || number == "1"
}

/// Whether a unit after `head` opens a clause, alone or after one of `LEADERS`.
fn opens_clause(head: &str) -> bool {
    let head = head.trim_end_matches(is_space);
    let head = (LEADERS.iter())
        .find_map(|leader| {
            let cut = head.len().checked_sub(leader.len())?;
            let word = head.get(cut..)?;
            let rest = &head[..cut];
            let bounded = !rest.ends_with(|c: char| is_word(c) || matches!(c, '.' | ','));
            (word.eq_ignore_ascii_case(leader) && bounded).then_some(rest)
        })
        .unwrap_or(head);
    let head = head.trim_end_matches(is_space);
    head.is_empty() || head.ends_with(CLAUSE_MARKS)
}

/// Where each full name of a decimal unit in `text`, in either number and any case, starts and
/// ends, with the unit's power.
fn full_names(text: &str) -> Vec<(usize, usize, u32)> {
    let mut spans = Vec::new();
    for (at, _) in text.char_indices() {
        for (k, name) in (1..).zip(DECIMAL_NAMES) {
            let Some(after) = strip_prefix_ignoring_case(&text[at..], name) else {
                continue;
            };
            let after = strip_prefix_ignoring_case(after, "s").unwrap_or(after);
            let end = text.len() - after.len();
            let before = text[..at].chars().next_back();
            if !before.is_some_and(is_word) && !after.starts_with(is_word) {
                spans.push((at, end, k));
            }
        }
    }
    spans
}

/// Whether `rest`, the text after a unit of power `k`, defines it: a defining word, then a count
/// of bytes equal to the unit's value.
fn defined_after_word(rest: &str, k: u32) -> bool {
    DEFINING.iter().any(|word| {
        strip_prefix_ignoring_case(rest, word).is_some_and(|after| {
            let boundary = matches!(*word, "=" | ":") || after.starts_with(is_space);
            let after = after.trim_start_matches(is_space);
            let after = strip_prefix_ignoring_case(after, "exactly")
                .map_or(after, |rest| rest.trim_start_matches(is_space));
            boundary && count::gives_value(after, k)
        })
    })
}

//! `V2-014` (F-21): a quantity stated in a decimal byte unit, such as `512 MB`, in a document
//! that also states one in a binary byte unit, such as `4 KiB`, and never says what its decimal
//! units mean. A reader then cannot tell whether `1 MB` is 1,000,000 bytes or 1,048,576.
//!
//! Every node and every decision is read, whatever the decision's state, as the text a Markdown
//! reader shows, so that code spans and fenced code state nothing. A quantity is a number
//! followed by a unit, with nothing, one space, one no-break space or one hyphen between them;
//! units are matched in their own case, so `Mb` is neither family. One definition of a decimal
//! unit settles every decimal unit of the document: a term named after one, or a text that gives
//! one as a count of bytes (`1 MB = 1,000,000 bytes`, `MB (10^6 bytes)`, `kB (kilobytes)`).

use super::context::{Context, Item};
use super::text::Body;
use super::{Raw, RuleId};
use crate::decision::{Decision, DecisionState, DriverPayload};
use crate::identity::Key;
use crate::node::Node;

const DECIMAL: [&str; 5] = ["kB", "KB", "MB", "GB", "TB"];
const BINARY: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
/// The full names of the decimal units, as a term may be named, in either number.
const DECIMAL_NAMES: [&str; 4] = ["kilobyte", "megabyte", "gigabyte", "terabyte"];
/// The words after a unit that make the text after them its definition.
const DEFINING: [&str; 6] = ["=", "is", "means", "equals", "denotes", "stands for"];

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
        let shown = Body::read(text);
        let shown = shown.text();
        for (at, unit) in units(shown) {
            if !is_quantity(shown, at) {
                continue;
            }
            if BINARY.contains(&unit) {
                self.binary = true;
            } else if self.decimal.is_none() {
                self.decimal = Some(unit);
            }
        }
        self.defines |= defines_decimal(shown);
    }

    /// A Quality Attribute's unit field: a byte unit there is a quantity in it.
    fn unit(&mut self, unit: &str) {
        let unit = unit.trim();
        if BINARY.contains(&unit) {
            self.binary = true;
        } else if let Some(decimal) = DECIMAL.iter().find(|each| **each == unit) {
            self.decimal = self.decimal.or(Some(decimal));
        }
    }
}

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    let document = context.document;
    let mut uses: Vec<(Key, Use)> = context
        .items
        .iter()
        .map(|item| (item.id.key(), node_use(item)))
        .collect();
    uses.extend(
        (document.decisions.iter()).map(|decision| (decision.id.key(), decision_use(decision))),
    );
    let defined = uses.iter().any(|(_, each)| each.defines)
        || document.terms.iter().any(|term| {
            names_decimal_unit(&term.name) || defines_decimal(Body::read(&term.definition).text())
        });
    let Some(binary) = uses
        .iter()
        .find(|(_, each)| each.binary)
        .map(|(key, _)| *key)
    else {
        return;
    };
    if defined {
        return;
    }
    for (key, each) in &uses {
        if let Some(unit) = each.decimal {
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

/// What a decision's every text says, whatever its state.
fn decision_use(decision: &Decision) -> Use {
    let mut each = Use::default();
    each.read(&decision.title);
    each.read(&decision.context);
    for driver in &decision.drivers {
        if let DriverPayload::Local { label, statement } = &driver.payload {
            each.read(label);
            each.read(statement);
        }
    }
    for option in &decision.options {
        each.read(&option.title);
        each.read(&option.description);
    }
    for assessment in &decision.assessments {
        each.read(&assessment.rationale);
    }
    for consequence in &decision.consequences {
        each.read(&consequence.statement);
    }
    match &decision.state {
        DecisionState::Proposed => {}
        DecisionState::Accepted { outcome, .. } | DecisionState::Superseded { outcome, .. } => {
            each.read(outcome);
        }
        DecisionState::Rejected { rationale } => each.read(rationale),
        DecisionState::Retired {
            outcome, rationale, ..
        } => {
            each.read(outcome);
            each.read(rationale);
        }
    }
    each
}

/// Whether a term's name is a decimal unit, or its full name in either number.
fn names_decimal_unit(name: &str) -> bool {
    let name = name.trim();
    if DECIMAL.contains(&name) {
        return true;
    }
    let name = name.to_lowercase();
    let singular = name.strip_suffix('s').unwrap_or(&name);
    DECIMAL_NAMES.contains(&singular)
}

/// Each unit in `text` that stands as a word of its own, with its byte offset.
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

/// Whether the unit at `at` follows a number, with nothing, one space, one no-break space or one
/// hyphen between them, and the number starts a word.
fn is_quantity(text: &str, at: usize) -> bool {
    let head = &text[..at];
    let head = head.strip_suffix([' ', '\u{a0}', '-']).unwrap_or(head);
    let rest = head.trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, ',' | '.'));
    let run = &head[rest.len()..];
    if !run.ends_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    let lead = run.len() - run.trim_start_matches([',', '.']).len();
    let before = if lead > 0 {
        run[..lead].chars().next_back()
    } else {
        rest.chars().next_back()
    };
    !before.is_some_and(is_word)
}

/// Whether `text` gives a decimal unit as a count of bytes, or names it in parentheses.
fn defines_decimal(text: &str) -> bool {
    units(text)
        .into_iter()
        .filter(|(_, unit)| DECIMAL.contains(unit))
        .any(|(at, unit)| definition_follows(text[at + unit.len()..].trim_start()))
}

/// Whether `rest`, the text after a unit, defines it: a defining word and a count of bytes, or a
/// parenthesis that opens on a count of bytes or a decimal unit's full name.
fn definition_follows(rest: &str) -> bool {
    if let Some(inside) = rest.strip_prefix('(') {
        let inside = inside.trim_start();
        let lower = inside.to_lowercase();
        return byte_count(inside) || DECIMAL_NAMES.iter().any(|name| lower.starts_with(name));
    }
    let lower = rest.to_lowercase();
    DEFINING.iter().any(|word| {
        lower.strip_prefix(word).is_some_and(|after| {
            let boundary = *word == "=" || after.starts_with(char::is_whitespace);
            let after = after.trim_start();
            let after = after.strip_prefix("exactly").map_or(after, str::trim_start);
            boundary && byte_count(after)
        })
    })
}

/// Whether `text` opens on a count of bytes: a number, or `10^n` or `2^n`, then `byte` or
/// `bytes` as a word.
fn byte_count(text: &str) -> bool {
    let rest = power(text).or_else(|| number(text));
    rest.is_some_and(|rest| {
        let lower = rest.trim_start().to_lowercase();
        let word = lower
            .strip_prefix("bytes")
            .or_else(|| lower.strip_prefix("byte"));
        word.is_some_and(|after| !after.starts_with(is_word))
    })
}

/// The text after a power such as `10^6` or `2 ^ 20` at the start of `text`.
fn power(text: &str) -> Option<&str> {
    let rest = text
        .strip_prefix("10")
        .or_else(|| text.strip_prefix('2'))?
        .trim_start()
        .strip_prefix('^')?
        .trim_start();
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    (digits > 0).then(|| &rest[digits..])
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

//! The language rules `LQ-001`..`LQ-009` (F-20), with the ids, severities and lexicons of
//! `loft-lint` 1.4.0. They read assertion texts (a Requirement's statement, a criterion's text, a
//! Constraint's or Quality Attribute's name) and the bodies of the same nodes (RQ-90, AC-651);
//! `LQ-008` also reads the names of Products, Feature Groups, Features and Components, and
//! `LQ-009` the names and bodies of every node that tags apply to. The text of a code span is
//! not prose: `LQ-001`..`LQ-005` and `LQ-009` leave it out of a name or statement, as every rule
//! leaves code out of a body, and `LQ-008` compares names whole.

use super::context::{Context, Item};
use super::text::{Body, occurrences, phrase_positions, shown_tokens, tokens};
use super::{LintConfig, Raw, RuleId};
use crate::identity::Key;
use crate::kind::NodeKind;
use crate::node::Node;
use crate::statement::Statement;

const LEXICAL: [(RuleId, &str); 4] = [
    (RuleId::Lq001, "a weak term"),
    (RuleId::Lq002, "a vague quantifier"),
    (RuleId::Lq003, "an escape clause"),
    (RuleId::Lq004, "absolute language"),
];

pub(super) fn check(context: &Context<'_>, config: &LintConfig, out: &mut Vec<Raw>) {
    for item in context
        .items
        .iter()
        .filter(|item| is_assertion(item.id.kind()))
    {
        let texts = Texts::read(item);
        for (rule, what) in LEXICAL {
            lexical(&texts, rule, what, config, out);
        }
        compound(&texts, config, out);
        outside_the_forms(item, out);
    }
    super::similarity::check(context, config, out);
    untagged_terms(context, out);
    // LQ-006 reads a Quality Attribute without a quantified target. Every Quality Attribute
    // carries a target in the model, so the rule has nothing to report.
}

/// A node whose text asserts something.
const fn is_assertion(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Requirement
            | NodeKind::Criterion
            | NodeKind::Constraint
            | NodeKind::QualityAttribute
    )
}

/// Where in a node's text a match is: its name or statement, or a line of its body.
#[derive(Clone, Copy)]
enum Spot {
    Name,
    Body(usize),
}

impl Spot {
    const fn line(self) -> Option<usize> {
        match self {
            Self::Name => None,
            Self::Body(line) => Some(line),
        }
    }
}

/// A node's texts, read once: its name's tokens, its body's tokens with their lines, and its
/// body's sentences with theirs.
struct Texts {
    key: Key,
    name: Vec<String>,
    body: Vec<String>,
    lines: Vec<usize>,
    sentences: Vec<(Vec<String>, usize)>,
}

impl Texts {
    fn read(item: &Item<'_>) -> Self {
        let body = item.body().map(Body::read);
        let (tokens_, lines) = body.as_ref().map(Body::tokens).unwrap_or_default();
        Self {
            key: item.id.key(),
            name: shown_tokens(&item.name()),
            body: tokens_,
            lines,
            sentences: body.as_ref().map(Body::sentences).unwrap_or_default(),
        }
    }

    /// Every spot `phrase` occurs at that `keep` keeps.
    fn spots<'a>(
        &'a self,
        phrase: &'a [String],
        keep: impl Fn(&[String], usize) -> bool + 'a,
    ) -> impl Iterator<Item = Spot> + 'a {
        let name = phrase_positions(&self.name, phrase)
            .into_iter()
            .filter(|at| keep(&self.name, *at))
            .map(|_| Spot::Name)
            .collect::<Vec<_>>();
        let body = phrase_positions(&self.body, phrase)
            .into_iter()
            .filter(move |at| keep(&self.body, *at))
            .map(|at| Spot::Body(self.lines[at]));
        name.into_iter().chain(body)
    }
}

/// `LQ-001`..`LQ-004`: one finding per occurrence of a lexicon phrase, naming the node and the
/// term (AC-373).
fn lexical(texts: &Texts, rule: RuleId, what: &str, config: &LintConfig, out: &mut Vec<Raw>) {
    for phrase in config.lexicon(rule) {
        let term = phrase.join(" ");
        let keep = |tokens: &[String], at| !exact_bound(rule, phrase, tokens, at);
        for spot in texts.spots(phrase, keep) {
            let message = format!("The text contains {what}.");
            out.push(
                Raw::new(rule, vec![texts.key], message)
                    .term(&term)
                    .line(spot.line()),
            );
        }
    }
}

/// `at most` followed by a number is a bound, not a vague quantifier (AC-378, AC-379).
fn exact_bound(rule: RuleId, phrase: &[String], tokens: &[String], at: usize) -> bool {
    rule == RuleId::Lq002
        && phrase == ["most"]
        && at > 0
        && tokens[at - 1] == "at"
        && tokens.get(at + 1).is_some_and(|token| is_quantity(token))
}

const NUMBER_WORDS: [&str; 28] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "thirty",
    "forty",
    "fifty",
    "sixty",
    "seventy",
    "eighty",
    "ninety",
];

/// Digits, digits grouped by commas in threes (`65,536`), or a number word.
fn is_quantity(token: &str) -> bool {
    let digits = |text: &str| !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
    let grouped = token.split_once(',').is_some_and(|(lead, groups)| {
        (1..=3).contains(&lead.len())
            && digits(lead)
            && groups
                .split(',')
                .all(|group| group.len() == 3 && digits(group))
    });
    digits(token) || grouped || NUMBER_WORDS.contains(&token)
}

/// `LQ-005`: a statement, or a sentence of its body, with more than one modal verb or with
/// conjunctions that reach the threshold; one finding per node (AC-387, AC-388).
fn compound(texts: &Texts, config: &LintConfig, out: &mut Vec<Raw>) {
    let compound = |tokens: &[String]| {
        occurrences(tokens, config.modals()) > 1
            || occurrences(tokens, config.conjunctions()) >= config.conjunction_threshold()
    };
    let spot = if compound(&texts.name) {
        Some(Spot::Name)
    } else {
        texts
            .sentences
            .iter()
            .find(|(tokens, _)| compound(tokens))
            .map(|(_, line)| Spot::Body(*line))
    };
    if let Some(spot) = spot {
        let message = "The text may hold more than one verifiable statement.";
        out.push(Raw::new(RuleId::Lq005, vec![texts.key], message).line(spot.line()));
    }
}

/// `LQ-007`: a Requirement whose statement is stored as text and parses as none of the five
/// forms (AC-394, AC-395).
fn outside_the_forms(item: &Item<'_>, out: &mut Vec<Raw>) {
    let Node::Requirement(requirement) = item.node else {
        return;
    };
    let Statement::Text(text) = &requirement.statement else {
        return;
    };
    if crate::ears::parse(text, &crate::text::token_ranges(text)).is_none() {
        out.push(Raw::new(
            RuleId::Lq007,
            vec![item.id.key()],
            "The statement matches none of the five EARS forms.",
        ));
    }
}

/// `LQ-009`: a registry term in the name or body of a node that tags apply to, where neither the
/// node nor a node above it lists the tag; one finding per node and term (AC-403, AC-404).
fn untagged_terms(context: &Context<'_>, out: &mut Vec<Raw>) {
    let mut entries: Vec<_> = context.document.tags.iter().collect();
    entries.sort_unstable_by_key(|entry| entry.id);
    for item in context.items.iter().filter(|item| taggable(item.id.kind())) {
        let name = shown_tokens(&item.name());
        let body = item.body().map(|body| Body::read(body).tokens());
        for entry in &entries {
            let phrase = tokens(entry.name.as_str());
            let covered = context
                .lineage(item.id)
                .any(|above| above.tags().any(|tag| tag == entry.name.as_str()));
            if phrase.is_empty() || covered {
                continue;
            }
            if let Some(spot) = first_spot(&phrase, &name, body.as_ref()) {
                out.push(
                    Raw::new(
                        RuleId::Lq009,
                        vec![item.id.key()],
                        "The text uses a tag registry term the node does not list.",
                    )
                    .term(entry.name.as_str())
                    .line(spot.line()),
                );
            }
        }
    }
}

/// The first spot `phrase` occurs at.
fn first_spot(
    phrase: &[String],
    name: &[String],
    body: Option<&(Vec<String>, Vec<usize>)>,
) -> Option<Spot> {
    if !phrase_positions(name, phrase).is_empty() {
        return Some(Spot::Name);
    }
    let (tokens, lines) = body?;
    let at = *phrase_positions(tokens, phrase).first()?;
    Some(Spot::Body(lines[at]))
}

const fn taggable(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Product
            | NodeKind::FeatureGroup
            | NodeKind::Feature
            | NodeKind::Requirement
            | NodeKind::Component
    )
}

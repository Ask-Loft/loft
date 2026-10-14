//! The language rules `LQ-001`..`LQ-009` (F-20), with the ids, severities and lexicons of
//! `loft-lint` 1.4.0. They read assertion texts (a Requirement's statement, a criterion's text, a
//! Constraint's or Quality Attribute's name) and the bodies of the same nodes (RQ-90, AC-651);
//! `LQ-008` also reads the names of Products, Feature Groups, Features and Components, and
//! `LQ-009` the names and bodies of every node that tags apply to. The text of a code span is
//! not prose: `LQ-001`..`LQ-005` and `LQ-009` leave it out of a name or statement, as every rule
//! leaves code out of a body, and `LQ-008` compares names whole.
//!
//! `V2-007` (F-21) is a lexical rule too, over the body of every node: its scan is the one
//! `LQ-001`..`LQ-004` use.

use super::context::{Context, Item};
use super::text::{Body, occurrences, phrase_positions, tokens, without_code_spans, words};
use super::{LintConfig, Raw, RuleId};
use crate::identity::Key;
use crate::kind::NodeKind;
use crate::node::Node;
use crate::statement::Statement;

/// The texts a lexical rule reads.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// An assertion's name or statement and its body.
    Assertion,
    /// The body of any node.
    Body,
}

/// Each lexical rule: the texts it reads and the message of its finding.
const LEXICAL: [(RuleId, Scope, &str); 5] = [
    (
        RuleId::Lq001,
        Scope::Assertion,
        "The text contains a weak term.",
    ),
    (
        RuleId::Lq002,
        Scope::Assertion,
        "The text contains a vague quantifier.",
    ),
    (
        RuleId::Lq003,
        Scope::Assertion,
        "The text contains an escape clause.",
    ),
    (
        RuleId::Lq004,
        Scope::Assertion,
        "The text contains absolute language.",
    ),
    (
        RuleId::V2007,
        Scope::Body,
        "The body holds a normative word; state the obligation in a statement or a criterion.",
    ),
];

pub(super) fn check(context: &Context<'_>, config: &LintConfig, out: &mut Vec<Raw>) {
    let terms = terms(context);
    for item in &context.items {
        let assertion = is_assertion(item.id.kind());
        let tagged = taggable(item.id.kind());
        if !assertion && !tagged && item.body().is_none() {
            continue;
        }
        let texts = Texts::read(item, assertion || tagged, assertion);
        for (rule, scope, message) in LEXICAL {
            if assertion || scope == Scope::Body {
                lexical(&texts, rule, scope, message, config, out);
            }
        }
        if assertion {
            compound(&texts, config, out);
            outside_the_forms(item, out);
        }
        if tagged {
            untagged_terms(context, item, &texts, &terms, out);
        }
    }
    super::similarity::check(context, config, out);
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

/// A node's texts, read once for every language rule: its body's words, its name's words where
/// a rule reads the name, and for an assertion its body's sentences with their lines.
struct Texts {
    key: Key,
    name: Words,
    body: Words,
    sentences: Vec<(Vec<String>, usize)>,
}

impl Texts {
    fn read(item: &Item<'_>, named: bool, assertion: bool) -> Self {
        let body = item.body().map(Body::read);
        let sentences = body.as_ref().filter(|_| assertion).map(Body::sentences);
        Self {
            key: item.id.key(),
            name: if named {
                Words::name(&item.name())
            } else {
                Words::default()
            },
            body: body.as_ref().map(Words::body).unwrap_or_default(),
            sentences: sentences.unwrap_or_default(),
        }
    }

    /// Every spot of `scope` that `phrase` occurs at and `keep` keeps.
    fn spots<'a>(
        &'a self,
        scope: Scope,
        phrase: &'a [String],
        keep: impl Fn(&[String], usize) -> bool + 'a,
    ) -> impl Iterator<Item = Spot> + 'a {
        let names = if scope == Scope::Assertion {
            phrase_positions(&self.name.tokens, phrase)
        } else {
            Vec::new()
        };
        let name = names
            .into_iter()
            .filter(|at| keep(&self.name.tokens, *at))
            .map(|_| Spot::Name)
            .collect::<Vec<_>>();
        let body = phrase_positions(&self.body.tokens, phrase)
            .into_iter()
            .filter(move |at| keep(&self.body.tokens, *at))
            .map(|at| Spot::Body(self.body.lines[at]));
        name.into_iter().chain(body)
    }
}

/// `LQ-001`..`LQ-004` and `V2-007`: one finding per occurrence of a lexicon phrase in the
/// rule's scope, naming the node and the term (AC-373).
fn lexical(
    texts: &Texts,
    rule: RuleId,
    scope: Scope,
    message: &str,
    config: &LintConfig,
    out: &mut Vec<Raw>,
) {
    for phrase in config.lexicon(rule) {
        let term = phrase.join(" ");
        let keep = |tokens: &[String], at| !exact_bound(rule, phrase, tokens, at);
        for spot in texts.spots(scope, phrase, keep) {
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
    let spot = if compound(&texts.name.tokens) {
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

/// A text's tokens, each token's body line (none for a name), and whether each token follows the
/// one before it across a hyphen.
#[derive(Default)]
struct Words {
    tokens: Vec<String>,
    lines: Vec<usize>,
    joins: Vec<bool>,
}

impl Words {
    fn name(name: &str) -> Self {
        let (tokens, joins) = words(&without_code_spans(name));
        Self {
            tokens,
            lines: Vec::new(),
            joins,
        }
    }

    fn body(body: &Body) -> Self {
        let (tokens, lines, joins) = body.words();
        Self {
            tokens,
            lines,
            joins,
        }
    }

    /// Where `phrase` first starts; a hyphenated tag only where its words are joined by hyphens
    /// in the text too.
    fn first(&self, phrase: &[String], hyphenated: bool) -> Option<usize> {
        phrase_positions(&self.tokens, phrase)
            .into_iter()
            .find(|at| !hyphenated || self.joins[at + 1..at + phrase.len()].iter().all(|j| *j))
    }
}

/// A tag registry entry as `LQ-009` matches it: its name, the name's tokens, and whether the
/// name joins its words by hyphens.
type Term<'a> = (&'a str, Vec<String>, bool);

/// The registry's terms in key order, each prepared once.
fn terms<'a>(context: &Context<'a>) -> Vec<Term<'a>> {
    let mut entries: Vec<_> = context.document.tags.iter().collect();
    entries.sort_unstable_by_key(|entry| entry.id);
    entries
        .into_iter()
        .map(|entry| {
            let name = entry.name.as_str();
            (name, tokens(name), name.contains('-'))
        })
        .filter(|(_, phrase, _)| !phrase.is_empty())
        .collect()
}

/// `LQ-009`: a registry term in the name or body of a node that tags apply to, where neither the
/// node nor a node above it lists the tag; one finding per node and term (AC-403, AC-404). A tag
/// whose words are joined by hyphens matches only text that joins them by hyphens.
fn untagged_terms(
    context: &Context<'_>,
    item: &Item<'_>,
    texts: &Texts,
    terms: &[Term<'_>],
    out: &mut Vec<Raw>,
) {
    let listed: Vec<&str> = context.lineage(item.id).flat_map(Item::tags).collect();
    for (term, phrase, hyphenated) in terms {
        if listed.contains(term) {
            continue;
        }
        if let Some(spot) = first_spot(phrase, *hyphenated, &texts.name, &texts.body) {
            out.push(
                Raw::new(
                    RuleId::Lq009,
                    vec![item.id.key()],
                    "The text uses a tag registry term the node does not list.",
                )
                .term(*term)
                .line(spot.line()),
            );
        }
    }
}

/// The first spot `phrase` occurs at.
fn first_spot(phrase: &[String], hyphenated: bool, name: &Words, body: &Words) -> Option<Spot> {
    if name.first(phrase, hyphenated).is_some() {
        return Some(Spot::Name);
    }
    let at = body.first(phrase, hyphenated)?;
    Some(Spot::Body(body.lines[at]))
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

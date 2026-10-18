//! The term report (the term reporter): candidate groups of phrases that may name one
//! concept, from `TD-001` (one qualifier before head nouns of one noun group), `TD-002` (one name
//! written as one word and as two or more) and `TD-003` (one phrase written with an abbreviation
//! and in full), and the subjects of each Product's statements. A group whose every phrase is a
//! term's name or alias is decided and left out. The report reads only the document and the lint
//! configuration, and `loft check` never runs its rules.

mod phrases;
mod subjects;

use std::collections::{BTreeMap, BTreeSet};

use super::context::Context;
use super::terms::phrase;
use super::{Groups, LintConfig, RuleId, Severity};
use crate::document::Document;
use crate::identity::Key;
use phrases::{Counted, PhraseKey};

/// What the term reporter yields for one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermReport {
    /// The candidate groups, most telling first.
    pub groups: Vec<Group>,
    /// Each Product's subjects, in the manifest's order.
    pub products: Vec<ProductSubjects>,
}

/// One group of phrases that may name one concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub rule: RuleId,
    pub severity: Severity,
    pub message: String,
    /// The phrases, most used first.
    pub phrases: Vec<Phrase>,
}

/// One phrase of a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    /// The form the text writes most often.
    pub text: String,
    /// How many times the text uses it.
    pub count: usize,
    /// Up to three nodes that use it, in document order.
    pub nodes: Vec<Key>,
}

/// The subjects of one Product's statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSubjects {
    pub product: Option<Key>,
    /// The subjects, most named first.
    pub subjects: Vec<Subject>,
}

/// One subject: the form written most often, without a leading article, and the number of
/// statements that name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    pub text: String,
    pub count: usize,
    pub mark: Option<Mark>,
}

/// What a subject is, where it is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    /// A term's name.
    Term,
    /// An alias of the term named.
    Alias(String),
    /// The name of a Product, Feature Group, Feature or Component.
    Node,
}

/// The groups a rule found: each with its message and its phrases' keys.
type Found = Vec<(String, Vec<PhraseKey>)>;

/// Every phrase of the prose, by its key.
type Phrases = BTreeMap<PhraseKey, Counted>;

/// A rule of the report: the groups it finds among the phrases.
type Rule = fn(&Phrases, &LintConfig) -> Found;

/// A group of phrases is reported when they occur this many times in all, for `TD-001` and
/// `TD-002`.
const LEAST_USES: usize = 3;

/// The term report of `document` under `config`.
#[must_use]
pub fn term_report(document: &Document, config: &LintConfig) -> TermReport {
    let context = Context::new(document);
    let phrases = phrases::collect(context.items.iter());
    let decided: BTreeSet<PhraseKey> = document
        .terms
        .iter()
        .flat_map(|term| std::iter::once(&term.name).chain(&term.aliases))
        .map(|name| phrase(name))
        .collect();
    let phrases = &phrases;
    let rules: [(RuleId, Rule); 3] = [
        (RuleId::Td001, head_nouns),
        (RuleId::Td002, spellings),
        (RuleId::Td003, abbreviations),
    ];
    let mut groups: Vec<Group> = rules
        .into_iter()
        .filter(|(rule, _)| config.enabled(*rule))
        .flat_map(|(rule, find)| {
            find(phrases, config)
                .into_iter()
                .filter(|(_, keys)| !keys.iter().all(|key| decided.contains(key)))
                .map(move |(message, keys)| group(rule, config, message, &keys, phrases))
                .collect::<Vec<_>>()
        })
        .collect();
    groups.sort_by(|left, right| {
        let second = |group: &Group| group.phrases.get(1).map_or(0, |phrase| phrase.count);
        second(right)
            .cmp(&second(left))
            .then_with(|| left.phrases[0].text.cmp(&right.phrases[0].text))
            .then(left.rule.cmp(&right.rule))
    });
    TermReport {
        groups,
        products: subjects::of(&context),
    }
}

fn group(
    rule: RuleId,
    config: &LintConfig,
    message: String,
    keys: &[PhraseKey],
    phrases: &Phrases,
) -> Group {
    let mut members: Vec<Phrase> = keys
        .iter()
        .map(|key| {
            let counted = &phrases[key];
            Phrase {
                text: counted.text().to_owned(),
                count: counted.count,
                nodes: counted.nodes.clone(),
            }
        })
        .collect();
    members.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.text.cmp(&right.text))
    });
    Group {
        rule,
        severity: config.severity(rule),
        message,
        phrases: members,
    }
}

fn uses(keys: &[PhraseKey], phrases: &Phrases) -> usize {
    keys.iter().map(|key| phrases[key].count).sum()
}

/// `TD-001`: phrases of two words with one qualifier, which is no noun group's word, and head
/// nouns of one noun group.
fn head_nouns(phrases: &Phrases, config: &LintConfig) -> Found {
    let groups = config.noun_groups();
    let in_a_group = |word: &str| groups.values().any(|words| words.contains(word));
    let mut by_qualifier: BTreeMap<(&str, &str), Vec<PhraseKey>> = BTreeMap::new();
    for key in phrases.keys() {
        let [qualifier, head] = key.as_slice() else {
            continue;
        };
        if in_a_group(qualifier) {
            continue;
        }
        for (name, words) in groups {
            if words.contains(head) {
                by_qualifier
                    .entry((qualifier, name))
                    .or_default()
                    .push(key.clone());
            }
        }
    }
    by_qualifier
        .into_iter()
        .filter(|(_, keys)| keys.len() > 1 && uses(keys, phrases) >= LEAST_USES)
        .map(|((qualifier, name), keys)| {
            let message =
                format!("`{qualifier}` is followed by head nouns of the noun group `{name}`.");
            (message, keys)
        })
        .collect()
}

/// `TD-002`: phrases equal once their spaces are removed, one a single word and one of more.
fn spellings(phrases: &Phrases, _: &LintConfig) -> Found {
    let mut by_letters: BTreeMap<String, Vec<PhraseKey>> = BTreeMap::new();
    for key in phrases.keys() {
        by_letters
            .entry(key.concat())
            .or_default()
            .push(key.clone());
    }
    by_letters
        .into_values()
        .filter(|keys| {
            keys.iter().any(|key| key.len() == 1)
                && keys.iter().any(|key| key.len() > 1)
                && uses(keys, phrases) >= LEAST_USES
        })
        .map(|keys| {
            let closed = keys
                .iter()
                .find(|key| key.len() == 1)
                .expect("a single word");
            let written = phrases[closed].text();
            let camel = written
                .chars()
                .zip(written.chars().skip(1))
                .any(|(a, b)| a.is_lowercase() && b.is_uppercase());
            let message = if camel {
                format!(
                    "`{written}` is an identifier written as prose: put it in a code span, or \
                     write the name one way."
                )
            } else {
                "One name is written as one word and as two or more.".to_owned()
            };
            (message, keys)
        })
        .collect()
}

/// `TD-003`: two phrases of two or three words, none a function word, equal once one word is
/// swapped for its long form; a phrase of three words only where no two of its words, the swapped
/// one among them, already form such a pair.
fn abbreviations(phrases: &Phrases, config: &LintConfig) -> Found {
    let candidates = phrases.keys().filter(|key| {
        (2..=3).contains(&key.len()) && !key.iter().any(|word| phrases::is_function_word(word))
    });
    let mut found = Found::new();
    for key in candidates {
        for (at, word, long) in swaps(key, config.pairs()) {
            let other = swapped(key, at, long);
            if phrases.contains_key(&other) && !holds_a_pair(phrases, key, at, long) {
                let message = format!("`{word}` abbreviates `{long}`.");
                found.push((message, vec![key.clone(), other]));
            }
        }
    }
    found
}

/// Each word of `key` that is a short form, with its place and each of its long forms.
fn swaps<'a>(key: &'a [String], pairs: &'a Groups) -> Vec<(usize, &'a str, &'a str)> {
    key.iter()
        .enumerate()
        .filter_map(|(at, word)| pairs.get(word).map(|longs| (at, word, longs)))
        .flat_map(|(at, word, longs)| {
            longs
                .iter()
                .map(move |long| (at, word.as_str(), long.as_str()))
        })
        .collect()
}

fn swapped(key: &[String], at: usize, long: &str) -> PhraseKey {
    let mut other = key.to_vec();
    long.clone_into(&mut other[at]);
    other
}

/// Whether two words of a three-word `key`, the one at `at` among them, form a pair already.
fn holds_a_pair(phrases: &Phrases, key: &[String], at: usize, long: &str) -> bool {
    key.len() == 3
        && [at.checked_sub(1), Some(at)]
            .into_iter()
            .flatten()
            .filter(|start| start + 2 <= key.len())
            .any(|start| {
                let pair = &key[start..start + 2];
                phrases.contains_key(pair) && phrases.contains_key(&swapped(pair, at - start, long))
            })
}

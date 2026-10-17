//! The phrases of a document's prose, as the term report counts them: every run of one to three
//! words in a clause, beginning and ending with a word that is not a function word. Code spans,
//! code blocks and punctuation end a clause; a hyphen between two words does not, so `change-set`
//! is the phrase `change set`.

use std::collections::BTreeMap;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use super::super::context::Item;
use super::super::defaults::FUNCTION_WORDS;
use super::super::terms::singular;
use crate::identity::Key;
use crate::text::{code_spans, lowercase, token_ranges};

/// The longest phrase the report counts, in words.
const LONGEST: usize = 3;

/// A phrase as compared: its words in lower case, the last one singular.
pub(super) type PhraseKey = Vec<String>;

/// One phrase's uses.
#[derive(Default)]
pub(super) struct Counted {
    pub count: usize,
    /// Each written form, its words in their own case joined by single spaces, with its count.
    pub forms: BTreeMap<String, usize>,
    /// The nodes that use it, in document order, each once, at most three.
    pub nodes: Vec<Key>,
}

impl Counted {
    /// The form written most often; of forms written equally often, the first in byte order.
    pub(super) fn text(&self) -> &str {
        self.forms
            .iter()
            .max_by(|left, right| left.1.cmp(right.1).then(right.0.cmp(left.0)))
            .map_or("", |(form, _)| form)
    }
}

/// Every phrase of the prose of `items`, by its key.
pub(super) fn collect<'a>(
    items: impl Iterator<Item = &'a Item<'a>>,
) -> BTreeMap<PhraseKey, Counted> {
    let mut phrases: BTreeMap<PhraseKey, Counted> = BTreeMap::new();
    for item in items {
        let name = clause_text_of_name(&item.name());
        let body = item.body().map(clause_text_of_body);
        for text in std::iter::once(name).chain(body) {
            for clause in clauses(&text) {
                add_runs(&clause, item.id.key(), &mut phrases);
            }
        }
    }
    phrases
}

/// A name or statement with each code span replaced by a line break, which ends a clause.
fn clause_text_of_name(name: &str) -> String {
    let mut shown = String::with_capacity(name.len());
    let mut copied = 0;
    for span in code_spans(name) {
        shown.push_str(&name[copied..span.start]);
        shown.push('\n');
        copied = span.end;
    }
    shown.push_str(&name[copied..]);
    shown
}

/// The text a Markdown reader shows of a body, without code blocks or HTML, with a line break
/// where a code span or a block stands, so that neither joins the words around it.
fn clause_text_of_body(markdown: &str) -> String {
    let mut text = String::new();
    let mut excluded = 0_u32;
    for event in Parser::new_ext(markdown, Options::empty()) {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) => excluded += 1,
            Event::End(TagEnd::CodeBlock | TagEnd::HtmlBlock) => excluded -= 1,
            Event::Text(piece) if excluded == 0 => text.push_str(&piece),
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            Event::Code(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item) => {
                text.push('\n');
            }
            _ => {}
        }
    }
    text
}

/// The clauses of a text: runs of words with only spaces, or one hyphen, between them. A word
/// that does not begin with a letter, or a possessive, ends a clause too.
fn clauses(text: &str) -> Vec<Vec<&str>> {
    let mut clauses = Vec::new();
    let mut clause: Vec<&str> = Vec::new();
    let mut end = 0;
    for range in token_ranges(text) {
        let gap = &text[end..range.start];
        let joined = gap == "-" || gap.chars().all(|c| c == ' ' || c == '\t');
        if !joined && !clause.is_empty() {
            clauses.push(std::mem::take(&mut clause));
        }
        end = range.end;
        let word = &text[range];
        // A possessive qualifies the noun after it as a name of its own owner, not as a
        // qualifier of one concept, so it ends the clause.
        let possessive = word.ends_with("'s") || word.ends_with("\u{2019}s");
        if word.chars().next().is_some_and(char::is_alphabetic) && !possessive {
            clause.push(word);
        } else if !clause.is_empty() {
            clauses.push(std::mem::take(&mut clause));
        }
    }
    if !clause.is_empty() {
        clauses.push(clause);
    }
    clauses
}

pub(super) fn is_function_word(word: &str) -> bool {
    FUNCTION_WORDS.binary_search(&word).is_ok()
}

/// Counts every run of a clause that is a phrase.
fn add_runs(clause: &[&str], node: Key, phrases: &mut BTreeMap<PhraseKey, Counted>) {
    let lower: Vec<String> = clause.iter().map(|word| lowercase(word)).collect();
    for start in 0..clause.len() {
        for length in 1..=LONGEST.min(clause.len() - start) {
            let words = &lower[start..start + length];
            if is_function_word(&words[0]) || is_function_word(&words[length - 1]) {
                continue;
            }
            let mut key = words.to_vec();
            let last = key.last_mut().expect("a run of at least one word");
            *last = singular(last);
            let counted = phrases.entry(key).or_default();
            counted.count += 1;
            *counted
                .forms
                .entry(clause[start..start + length].join(" "))
                .or_default() += 1;
            if counted.nodes.len() < 3 && !counted.nodes.contains(&node) {
                counted.nodes.push(node);
            }
        }
    }
}

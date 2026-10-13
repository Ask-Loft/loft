//! The lint configuration in the repository (F-23, ADR-11): `spec/lint.yaml` disables rules, sets
//! severities, edits lexicons, sets thresholds and records suppressions. Without the file the
//! defaults apply (AC-426). A rule, severity or field the Format does not define is an error at
//! its file, line and column, and no finding is reported then (RQ-174).

use std::collections::{BTreeMap, BTreeSet};

use super::defaults::{
    self, CONJUNCTION_THRESHOLD, CONJUNCTIONS, MAX_BODY_WORDS, MAX_CRITERIA, MIN_TOKENS, MODALS,
    SIMILARITY_MILLIONTHS,
};
use super::lexicon::{self, Edits, Lexicon};
use super::{RuleId, Severity};
use crate::codec::{LINT, Problem, ReadError, SpecPath};
use crate::identity::Key;
use crate::yaml::{self, Entry, Node, Position, Value};

/// LQ-008's comparison: names of at least `min_tokens` tokens are near-duplicates when their
/// similarity reaches `threshold` millionths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Similarity {
    pub min_tokens: usize,
    pub threshold: u32,
}

/// An effective lint configuration: every rule's switch and severity, every lexicon and
/// threshold, and the suppressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintConfig {
    rules: BTreeMap<RuleId, (bool, Severity)>,
    lexicons: BTreeMap<RuleId, Lexicon>,
    modals: Lexicon,
    conjunctions: Lexicon,
    conjunction_threshold: usize,
    similarity: Similarity,
    max_criteria: usize,
    max_body_words: usize,
    proper_names: Vec<String>,
    suppressions: BTreeSet<(RuleId, Key)>,
}

impl Default for LintConfig {
    fn default() -> Self {
        let lexicon = |terms: &[&str]| lexicon::effective(terms, None).expect("a valid default");
        Self {
            rules: RuleId::ALL
                .iter()
                .map(|rule| (*rule, (rule.enabled_by_default(), rule.default_severity())))
                .collect(),
            lexicons: RuleId::ALL
                .iter()
                .filter_map(|rule| Some((*rule, lexicon(defaults::lexicon(*rule)?))))
                .collect(),
            modals: lexicon(MODALS),
            conjunctions: lexicon(CONJUNCTIONS),
            conjunction_threshold: CONJUNCTION_THRESHOLD,
            similarity: Similarity {
                min_tokens: MIN_TOKENS,
                threshold: SIMILARITY_MILLIONTHS,
            },
            max_criteria: MAX_CRITERIA,
            max_body_words: MAX_BODY_WORDS,
            proper_names: Vec::new(),
            suppressions: BTreeSet::new(),
        }
    }
}

impl LintConfig {
    /// Reads `spec/lint.yaml`; no file means the defaults.
    ///
    /// # Errors
    ///
    /// Every rule, severity, field or value the file states that the Format does not define,
    /// each at its line and column.
    pub fn read(bytes: Option<&[u8]>) -> Result<Self, Vec<ReadError>> {
        let path = SpecPath::new(LINT);
        let mut config = Self::default();
        let Some(bytes) = bytes else {
            return Ok(config);
        };
        let root = yaml::read(bytes).map_err(|error| vec![ReadError::yaml(&path, error)])?;
        let mut reader = Reader {
            path,
            errors: Vec::new(),
        };
        if let Some(root) = root {
            reader.file(&root, &mut config);
        }
        if reader.errors.is_empty() {
            Ok(config)
        } else {
            Err(reader.errors)
        }
    }

    #[must_use]
    pub fn enabled(&self, rule: RuleId) -> bool {
        self.rules[&rule].0
    }

    #[must_use]
    pub fn severity(&self, rule: RuleId) -> Severity {
        self.rules[&rule].1
    }

    /// Whether a suppression names `rule` and one of `keys` (AC-675).
    #[must_use]
    pub fn suppresses(&self, rule: RuleId, keys: &[Key]) -> bool {
        keys.iter()
            .any(|key| self.suppressions.contains(&(rule, *key)))
    }

    /// The lexicon of a rule that takes one: `LQ-001`..`LQ-004` and `V2-007`.
    #[must_use]
    pub fn lexicon(&self, rule: RuleId) -> &Lexicon {
        &self.lexicons[&rule]
    }

    #[must_use]
    pub fn modals(&self) -> &Lexicon {
        &self.modals
    }

    #[must_use]
    pub fn conjunctions(&self) -> &Lexicon {
        &self.conjunctions
    }

    #[must_use]
    pub const fn conjunction_threshold(&self) -> usize {
        self.conjunction_threshold
    }

    #[must_use]
    pub const fn similarity(&self) -> Similarity {
        self.similarity
    }

    /// `V2-006`'s limit on a Requirement's criteria.
    #[must_use]
    pub const fn max_criteria(&self) -> usize {
        self.max_criteria
    }

    /// `V2-006`'s limit on the words of a Requirement's body.
    #[must_use]
    pub const fn max_body_words(&self) -> usize {
        self.max_body_words
    }

    /// The proper names `V2-008` reads, each matched whole and in its own case.
    #[must_use]
    pub fn proper_names(&self) -> &[String] {
        &self.proper_names
    }
}

/// A threshold in millionths, written as a decimal in (0, 1] with at most six places.
pub(super) fn millionths_text(millionths: u32) -> String {
    let text = format!("{}.{:06}", millionths / 1_000_000, millionths % 1_000_000);
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn parse_millionths(text: &str) -> Option<u32> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    if whole.is_empty() || !digits(whole) || !digits(fraction) || fraction.len() > 6 {
        return None;
    }
    let whole: u32 = whole.parse().ok()?;
    let fraction: u32 = format!("{fraction:0<6}").parse().ok()?;
    let value = whole.checked_mul(1_000_000)?.checked_add(fraction)?;
    (1..=1_000_000).contains(&value).then_some(value)
}

struct Reader {
    path: SpecPath,
    errors: Vec<ReadError>,
}

impl Reader {
    fn fail(&mut self, position: Position, problem: Problem) {
        self.errors
            .push(ReadError::at(&self.path, Some(position), problem));
    }

    fn entries<'a>(&mut self, node: &'a Node) -> &'a [Entry] {
        if let Value::Mapping(entries) = &node.value {
            return entries;
        }
        self.fail(node.position, Problem::Expected("a mapping"));
        &[]
    }

    /// A sequence's items; a field written with no value holds none.
    fn items<'a>(&mut self, node: &'a Node) -> &'a [Node] {
        match &node.value {
            Value::Sequence(items) => items,
            Value::Empty => &[],
            _ => {
                self.fail(node.position, Problem::Expected("a sequence"));
                &[]
            }
        }
    }

    fn scalar<'a>(&mut self, node: &'a Node) -> Option<&'a str> {
        if let Value::Scalar(scalar) = &node.value {
            return Some(&scalar.text);
        }
        self.fail(node.position, Problem::Expected("a scalar"));
        None
    }

    fn file(&mut self, root: &Node, config: &mut LintConfig) {
        for entry in self.entries(root) {
            match entry.key.as_str() {
                "rules" => self.rules(&entry.value, config),
                "suppressions" => self.suppressions(&entry.value, config),
                other => self.fail(entry.key_position, Problem::UnknownField(other.to_owned())),
            }
        }
    }

    fn rules(&mut self, node: &Node, config: &mut LintConfig) {
        for item in self.items(node) {
            self.rule(item, config);
        }
    }

    fn suppressions(&mut self, node: &Node, config: &mut LintConfig) {
        for item in self.items(node) {
            self.suppression(item, config);
        }
    }

    /// A rule's entry: `rule`, its id, then the options it sets.
    fn rule(&mut self, item: &Node, config: &mut LintConfig) {
        let entries = self.entries(item);
        let Some(named) = entries.iter().find(|entry| entry.key == "rule") else {
            if !entries.is_empty() {
                self.fail(item.position, Problem::MissingField("rule"));
            }
            return;
        };
        let Some(rule) = self.parsed(&named.value, "a rule the Format defines", RuleId::parse)
        else {
            return;
        };
        for option in entries.iter().filter(|entry| entry.key != "rule") {
            self.option(rule, option, config);
        }
    }

    fn option(&mut self, rule: RuleId, option: &Entry, config: &mut LintConfig) {
        let value = &option.value;
        match (option.key.as_str(), rule) {
            ("enabled", _) => {
                if let Some(enabled) = self.parsed(value, "`true` or `false`", |text| match text {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                }) {
                    config.rules.get_mut(&rule).expect("every rule").0 = enabled;
                }
            }
            ("severity", _) => {
                if let Some(severity) = self.parsed(value, "warning, info or hint", Severity::parse)
                {
                    config.rules.get_mut(&rule).expect("every rule").1 = severity;
                }
            }
            ("lexicon", _) if defaults::lexicon(rule).is_some() => {
                let defaults = defaults::lexicon(rule).expect("a lexical rule");
                if let Some(lexicon) = self.lexicon(value, defaults) {
                    config.lexicons.insert(rule, lexicon);
                }
            }
            ("modalLexicon", RuleId::Lq005) => {
                if let Some(lexicon) = self.lexicon(value, MODALS) {
                    config.modals = lexicon;
                }
            }
            ("conjunctionLexicon", RuleId::Lq005) => {
                if let Some(lexicon) = self.lexicon(value, CONJUNCTIONS) {
                    config.conjunctions = lexicon;
                }
            }
            ("conjunctionThreshold", RuleId::Lq005) => {
                let at_least_two = |text: &str| text.parse().ok().filter(|count| *count >= 2);
                if let Some(count) = self.parsed(value, "a count of at least 2", at_least_two) {
                    config.conjunction_threshold = count;
                }
            }
            ("minTokens", RuleId::Lq008) => {
                if let Some(count) = self.count(value) {
                    config.similarity.min_tokens = count;
                }
            }
            ("threshold", RuleId::Lq008) => {
                let expected = "a decimal above 0 and at most 1, with at most six places";
                if let Some(threshold) = self.parsed(value, expected, parse_millionths) {
                    config.similarity.threshold = threshold;
                }
            }
            ("maxCriteria", RuleId::V2006) => {
                if let Some(count) = self.count(value) {
                    config.max_criteria = count;
                }
            }
            ("maxBodyWords", RuleId::V2006) => {
                if let Some(count) = self.count(value) {
                    config.max_body_words = count;
                }
            }
            ("properNames", RuleId::V2008) => {
                if let Some(names) = self.names(value) {
                    config.proper_names = names;
                }
            }
            (other, _) => self.fail(option.key_position, Problem::UnknownField(other.to_owned())),
        }
    }

    fn parsed<T>(
        &mut self,
        node: &Node,
        expected: &'static str,
        parse: impl Fn(&str) -> Option<T>,
    ) -> Option<T> {
        let text = self.scalar(node)?;
        let parsed = parse(text);
        if parsed.is_none() {
            let value = text.to_owned();
            self.fail(node.position, Problem::Invalid { value, expected });
        }
        parsed
    }

    fn count(&mut self, node: &Node) -> Option<usize> {
        let positive = |text: &str| text.parse().ok().filter(|count| *count >= 1);
        self.parsed(node, "a count of at least 1", positive)
    }

    /// A sequence of names, none of them blank; each blank name is an error.
    fn names(&mut self, node: &Node) -> Option<Vec<String>> {
        let errors = self.errors.len();
        let names: Vec<String> = self
            .items(node)
            .iter()
            .filter_map(|item| {
                self.parsed(item, "a name", |text| {
                    (!text.trim().is_empty()).then(|| text.to_owned())
                })
            })
            .collect();
        (self.errors.len() == errors).then_some(names)
    }

    fn lexicon(&mut self, node: &Node, defaults: &[&str]) -> Option<Lexicon> {
        let mut edits = Edits::default();
        for entry in self.entries(node) {
            let mut phrases = Vec::new();
            for item in self.items(&entry.value) {
                phrases.extend(self.scalar(item).map(str::to_owned));
            }
            match entry.key.as_str() {
                "replace" => edits.replace = Some(phrases),
                "extend" => edits.extend = phrases,
                "remove" => edits.remove = phrases,
                other => self.fail(entry.key_position, Problem::UnknownField(other.to_owned())),
            }
        }
        lexicon::effective(defaults, Some(&edits))
            .map_err(|problem| self.fail(node.position, Problem::Shape(problem)))
            .ok()
    }

    fn suppression(&mut self, item: &Node, config: &mut LintConfig) {
        let entries = self.entries(item);
        let field = |name: &str| entries.iter().find(|entry| entry.key == name);
        for entry in entries {
            if !matches!(entry.key.as_str(), "rule" | "key" | "reason") {
                self.fail(entry.key_position, Problem::UnknownField(entry.key.clone()));
            }
        }
        let mut required = |name: &'static str| {
            let found = field(name);
            if found.is_none() {
                self.errors.push(ReadError::at(
                    &self.path,
                    Some(item.position),
                    Problem::MissingField(name),
                ));
            }
            found
        };
        let (rule, key, reason) = (required("rule"), required("key"), required("reason"));
        let rule = rule
            .and_then(|rule| self.parsed(&rule.value, "a rule the Format defines", RuleId::parse));
        let key =
            key.and_then(|key| self.parsed(&key.value, "a key", |text| Key::parse(text).ok()));
        let reason = reason.and_then(|reason| {
            self.parsed(&reason.value, "a reason", |text| {
                (!text.trim().is_empty()).then_some(())
            })
        });
        if let (Some(rule), Some(key), Some(())) = (rule, key, reason) {
            config.suppressions.insert((rule, key));
        }
    }
}

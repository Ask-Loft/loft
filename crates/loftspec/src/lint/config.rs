//! The lint configuration in the repository (F-23, ADR-11): `spec/lint.yaml` disables rules, sets
//! severities, edits lexicons, sets thresholds and records suppressions. Without the file the
//! defaults apply (AC-426). A rule, severity or field the Format does not define is an error at
//! its file, line and column, and no finding is reported then (RQ-174).

use std::collections::{BTreeMap, BTreeSet};

use super::defaults::{self, Kind, OPTIONS};
use super::lexicon::{self, Edits, Lexicon};
use super::{RuleId, Severity};
use crate::codec::{LINT, Problem, ReadError, SpecPath, Strict};
use crate::identity::Key;
use crate::yaml::{self, Entry, Node, Position};

/// LQ-008's comparison: names of at least `min_tokens` tokens are near-duplicates when their
/// similarity reaches `threshold` millionths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Similarity {
    pub min_tokens: usize,
    pub threshold: u32,
}

/// The value of a rule's option, of the [`Kind`] the option takes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Setting {
    Lexicon(Lexicon),
    Count(usize),
    Millionths(u32),
    Names(Vec<String>),
}

impl Setting {
    fn default_of(kind: &Kind) -> Self {
        match kind {
            Kind::Lexicon(terms) => {
                Self::Lexicon(lexicon::effective(terms, None).expect("a valid default"))
            }
            Kind::Count { default, .. } => Self::Count(*default),
            Kind::Millionths(default) => Self::Millionths(*default),
            Kind::Names => Self::Names(Vec::new()),
        }
    }
}

/// An effective lint configuration: every rule's switch and severity, every option, and the
/// suppressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintConfig {
    rules: BTreeMap<RuleId, (bool, Severity)>,
    options: BTreeMap<(RuleId, &'static str), Setting>,
    suppressions: BTreeSet<(RuleId, Key)>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            rules: RuleId::ALL
                .iter()
                .map(|rule| (*rule, (rule.enabled_by_default(), rule.default_severity())))
                .collect(),
            options: OPTIONS
                .iter()
                .map(|(rule, key, kind)| ((*rule, *key), Setting::default_of(kind)))
                .collect(),
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
        self.lexicon_at(rule, "lexicon")
    }

    #[must_use]
    pub fn modals(&self) -> &Lexicon {
        self.lexicon_at(RuleId::Lq005, "modalLexicon")
    }

    #[must_use]
    pub fn conjunctions(&self) -> &Lexicon {
        self.lexicon_at(RuleId::Lq005, "conjunctionLexicon")
    }

    #[must_use]
    pub fn conjunction_threshold(&self) -> usize {
        self.count(RuleId::Lq005, "conjunctionThreshold")
    }

    #[must_use]
    pub fn similarity(&self) -> Similarity {
        let Setting::Millionths(threshold) = self.options[&(RuleId::Lq008, "threshold")] else {
            unreachable!("`threshold` is a decimal");
        };
        Similarity {
            min_tokens: self.count(RuleId::Lq008, "minTokens"),
            threshold,
        }
    }

    /// `V2-006`'s limit on a Requirement's criteria.
    #[must_use]
    pub fn max_criteria(&self) -> usize {
        self.count(RuleId::V2006, "maxCriteria")
    }

    /// `V2-006`'s limit on the words of a Requirement's body.
    #[must_use]
    pub fn max_body_words(&self) -> usize {
        self.count(RuleId::V2006, "maxBodyWords")
    }

    /// The proper names `V2-008` reads, each matched whole and in its own case.
    #[must_use]
    pub fn proper_names(&self) -> &[String] {
        let Setting::Names(names) = &self.options[&(RuleId::V2008, "properNames")] else {
            unreachable!("`properNames` is a list of names");
        };
        names
    }

    // Each option holds the setting of its kind: the defaults and the reader both follow
    // `OPTIONS`.
    fn lexicon_at(&self, rule: RuleId, key: &'static str) -> &Lexicon {
        let Setting::Lexicon(lexicon) = &self.options[&(rule, key)] else {
            unreachable!("`{key}` is a lexicon");
        };
        lexicon
    }

    fn count(&self, rule: RuleId, key: &'static str) -> usize {
        let Setting::Count(count) = self.options[&(rule, key)] else {
            unreachable!("`{key}` is a count");
        };
        count
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

impl Strict for Reader {
    fn fail(&mut self, position: Option<Position>, problem: Problem) {
        self.errors
            .push(ReadError::at(&self.path, position, problem));
    }
}

impl Reader {
    fn file(&mut self, root: &Node, config: &mut LintConfig) {
        for entry in self.entries(root) {
            match entry.key.as_str() {
                "rules" => self.rules(&entry.value, config),
                "suppressions" => self.suppressions(&entry.value, config),
                other => {
                    let unknown = Problem::UnknownField(other.to_owned());
                    self.fail(Some(entry.key_position), unknown);
                }
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
                self.fail(Some(item.position), Problem::MissingField("rule"));
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
        match option.key.as_str() {
            "enabled" => {
                if let Some(enabled) = self.parsed(value, "`true` or `false`", |text| match text {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                }) {
                    config.rules.get_mut(&rule).expect("every rule").0 = enabled;
                }
            }
            "severity" => {
                if let Some(severity) = self.parsed(value, "warning, info or hint", Severity::parse)
                {
                    config.rules.get_mut(&rule).expect("every rule").1 = severity;
                }
            }
            key => {
                let Some((key, kind)) = defaults::option(rule, key) else {
                    let unknown = Problem::UnknownField(key.to_owned());
                    return self.fail(Some(option.key_position), unknown);
                };
                if let Some(setting) = self.setting(value, kind) {
                    config.options.insert((rule, key), setting);
                }
            }
        }
    }

    /// An option's setting, read as its kind reads.
    fn setting(&mut self, node: &Node, kind: &Kind) -> Option<Setting> {
        match kind {
            Kind::Lexicon(defaults) => self.lexicon(node, defaults).map(Setting::Lexicon),
            Kind::Count {
                least, expected, ..
            } => {
                let count = |text: &str| text.parse().ok().filter(|count| count >= least);
                self.parsed(node, expected, count).map(Setting::Count)
            }
            Kind::Millionths(_) => {
                let expected = "a decimal above 0 and at most 1, with at most six places";
                self.parsed(node, expected, parse_millionths)
                    .map(Setting::Millionths)
            }
            Kind::Names => self.names(node).map(Setting::Names),
        }
    }

    /// A sequence of names, none of them blank; each blank name is an error.
    fn names(&mut self, node: &Node) -> Option<Vec<String>> {
        let errors = self.errors.len();
        let names: Vec<String> = self
            .items(node)
            .iter()
            .filter_map(|item| self.text(item, "a name"))
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
                other => {
                    let unknown = Problem::UnknownField(other.to_owned());
                    self.fail(Some(entry.key_position), unknown);
                }
            }
        }
        lexicon::effective(defaults, Some(&edits))
            .map_err(|problem| self.fail(Some(node.position), Problem::Shape(problem)))
            .ok()
    }

    fn suppression(&mut self, item: &Node, config: &mut LintConfig) {
        let fields = self.fields(item, &["rule", "key", "reason"]);
        let (rule, key, reason) = (
            self.required(item, fields, "rule"),
            self.required(item, fields, "key"),
            self.required(item, fields, "reason"),
        );
        let rule = rule
            .and_then(|rule| self.parsed(&rule.value, "a rule the Format defines", RuleId::parse));
        let key =
            key.and_then(|key| self.parsed(&key.value, "a key", |text| Key::parse(text).ok()));
        let reason = reason.and_then(|reason| self.text(&reason.value, "a reason"));
        if let (Some(rule), Some(key), Some(_)) = (rule, key, reason) {
            config.suppressions.insert((rule, key));
        }
    }
}

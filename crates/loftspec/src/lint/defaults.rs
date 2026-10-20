//! The default configuration the Format publishes (AC-374, AC-389, AC-401, AC-647..AC-650): each
//! rule's severity and whether it runs, every lexicon, and the thresholds. The same values are
//! published as `testdata/golden/lint-defaults.yaml`, a `spec/lint.yaml` that states every
//! default, which [`defaults_text`] writes.

use std::fmt::Write;

use super::{RuleId, Severity};

pub(super) const WEAK_TERMS: &[&str] = &[
    "appropriate",
    "adequate",
    "efficient",
    "flexible",
    "user friendly",
    "easy",
    "fast",
    "seamless",
    "reasonable",
    "sufficient",
    "robust",
    "intuitive",
    "timely",
    "optimal",
    "as soon as possible",
];

pub(super) const VAGUE_QUANTIFIERS: &[&str] = &[
    "some",
    "several",
    "many",
    "few",
    "most",
    "various",
    "significant",
    "minimal",
    "frequently",
    "rarely",
    "regularly",
];

pub(super) const ESCAPE_CLAUSES: &[&str] = &[
    "if possible",
    "where appropriate",
    "as appropriate",
    "as required",
    "as necessary",
    "if needed",
    "where feasible",
    "to the extent practical",
    "if applicable",
];

pub(super) const ABSOLUTES: &[&str] = &["all", "always", "never", "every", "any", "none"];

pub(super) const MODALS: &[&str] = &["shall", "must", "will", "should"];

pub(super) const CONJUNCTIONS: &[&str] = &["and", "or"];

/// The words `V2-007` reads as normative in a body.
pub(super) const NORMATIVE_WORDS: &[&str] = &["shall", "must"];

/// The noun groups of `TD-001`: head nouns that a text may use for one another after the same
/// qualifier.
pub(super) const NOUN_GROUPS: &[(&str, &[&str])] = &[
    (
        "record",
        &[
            "record", "entry", "item", "row", "object", "entity", "element",
        ],
    ),
    (
        "view",
        &["view", "page", "screen", "panel", "pane", "tab", "surface"],
    ),
    (
        "identifier",
        &[
            "id",
            "identifier",
            "key",
            "label",
            "ref",
            "reference",
            "handle",
            "slug",
        ],
    ),
    (
        "failure",
        &["error", "failure", "refusal", "rejection", "fault"],
    ),
    (
        "request",
        &[
            "request",
            "call",
            "invocation",
            "operation",
            "command",
            "action",
        ],
    ),
    ("document", &["document", "doc", "spec", "specification"]),
    (
        "system",
        &[
            "service",
            "server",
            "engine",
            "subsystem",
            "module",
            "layer",
            "system",
            "backend",
            "adapter",
            "worker",
            "daemon",
        ],
    ),
];

/// The abbreviations of `TD-003`, each named by its short form, with its long forms.
pub(super) const PAIRS: &[(&str, &[&str])] = &[
    ("admin", &["administrator", "administration"]),
    ("app", &["application"]),
    ("arg", &["argument"]),
    ("config", &["configuration"]),
    ("db", &["database"]),
    ("dev", &["development"]),
    ("dir", &["directory"]),
    ("doc", &["document"]),
    ("env", &["environment"]),
    ("id", &["identifier"]),
    ("impl", &["implementation"]),
    ("info", &["information"]),
    ("msg", &["message"]),
    ("param", &["parameter"]),
    ("prod", &["production"]),
    ("repo", &["repository"]),
    ("spec", &["specification"]),
];

/// The words a phrase of the term report neither begins nor ends with, published as
/// `testdata/golden/function-words.txt`.
pub const FUNCTION_WORDS: &[&str] = &[
    "a",
    "about",
    "across",
    "after",
    "against",
    "all",
    "also",
    "although",
    "am",
    "among",
    "an",
    "and",
    "another",
    "any",
    "are",
    "as",
    "at",
    "be",
    "because",
    "been",
    "before",
    "being",
    "between",
    "both",
    "but",
    "by",
    "can",
    "cannot",
    "could",
    "did",
    "different",
    "do",
    "does",
    "done",
    "down",
    "during",
    "each",
    "eight",
    "either",
    "else",
    "every",
    "few",
    "first",
    "five",
    "for",
    "four",
    "from",
    "given",
    "had",
    "has",
    "have",
    "having",
    "he",
    "her",
    "here",
    "his",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "it",
    "its",
    "just",
    "last",
    "least",
    "less",
    "many",
    "may",
    "me",
    "might",
    "more",
    "most",
    "must",
    "my",
    "neither",
    "new",
    "next",
    "nine",
    "no",
    "nor",
    "not",
    "of",
    "off",
    "on",
    "once",
    "one",
    "only",
    "onto",
    "or",
    "other",
    "our",
    "out",
    "over",
    "own",
    "per",
    "previous",
    "same",
    "seven",
    "shall",
    "she",
    "should",
    "since",
    "six",
    "so",
    "some",
    "such",
    "ten",
    "than",
    "that",
    "the",
    "their",
    "them",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "though",
    "three",
    "through",
    "to",
    "two",
    "under",
    "unless",
    "until",
    "up",
    "upon",
    "us",
    "very",
    "via",
    "was",
    "we",
    "were",
    "what",
    "when",
    "where",
    "whether",
    "which",
    "while",
    "who",
    "whole",
    "whom",
    "whose",
    "why",
    "will",
    "with",
    "within",
    "without",
    "would",
    "yet",
    "you",
    "your",
    "zero",
];

/// What an option holds, and its default.
pub(super) enum Kind {
    /// A lexicon, edited by `replace` or by `extend` and `remove`.
    Lexicon(&'static [&'static str]),
    /// A whole number of at least `least`, which `expected` names in an error.
    Count {
        default: usize,
        least: usize,
        expected: &'static str,
    },
    /// A decimal above 0 and at most 1, with at most six places, held in millionths.
    Millionths(u32),
    /// A list of names, none of them blank; none by default.
    Names,
    /// Named groups of words, edited by `replace` or by `extend` and `remove`.
    Groups(&'static [(&'static str, &'static [&'static str])]),
}

/// Every option a rule takes beside `enabled` and `severity`, by rule and key, in the order the
/// published defaults state them.
pub(super) const OPTIONS: [(RuleId, &str, Kind); 15] = [
    (RuleId::Lq001, "lexicon", Kind::Lexicon(WEAK_TERMS)),
    (RuleId::Lq002, "lexicon", Kind::Lexicon(VAGUE_QUANTIFIERS)),
    (RuleId::Lq003, "lexicon", Kind::Lexicon(ESCAPE_CLAUSES)),
    (RuleId::Lq004, "lexicon", Kind::Lexicon(ABSOLUTES)),
    (RuleId::Lq005, "modalLexicon", Kind::Lexicon(MODALS)),
    (
        RuleId::Lq005,
        "conjunctionLexicon",
        Kind::Lexicon(CONJUNCTIONS),
    ),
    // A segment whose conjunctions reach this count.
    (
        RuleId::Lq005,
        "conjunctionThreshold",
        Kind::Count {
            default: 3,
            least: 2,
            expected: "a count of at least 2",
        },
    ),
    // Names of at least this many tokens are compared.
    (
        RuleId::Lq008,
        "minTokens",
        Kind::Count {
            default: 2,
            least: 1,
            expected: "a count of at least 1",
        },
    ),
    // Two names whose similarity reaches this are near-duplicates.
    (RuleId::Lq008, "threshold", Kind::Millionths(800_000)),
    // A Requirement with more criteria than this.
    (
        RuleId::V2006,
        "maxCriteria",
        Kind::Count {
            default: 20,
            least: 1,
            expected: "a count of at least 1",
        },
    ),
    // A Requirement whose body holds more words than this.
    (
        RuleId::V2006,
        "maxBodyWords",
        Kind::Count {
            default: 150,
            least: 1,
            expected: "a count of at least 1",
        },
    ),
    (RuleId::V2007, "lexicon", Kind::Lexicon(NORMATIVE_WORDS)),
    (RuleId::V2008, "properNames", Kind::Names),
    (RuleId::Td001, "nounGroups", Kind::Groups(NOUN_GROUPS)),
    (RuleId::Td003, "pairs", Kind::Groups(PAIRS)),
];

/// The option `key` of `rule`, where the rule takes it.
pub(super) fn option(rule: RuleId, key: &str) -> Option<(&'static str, &'static Kind)> {
    OPTIONS
        .iter()
        .find(|(each, name, _)| *each == rule && *name == key)
        .map(|(_, name, kind)| (*name, kind))
}

impl RuleId {
    /// Whether the rule runs when the configuration does not say (AC-385, AC-397).
    #[must_use]
    pub const fn enabled_by_default(self) -> bool {
        !matches!(self, Self::Lq004 | Self::Lq007)
    }

    /// The rule's severity when the configuration does not set one.
    #[must_use]
    pub const fn default_severity(self) -> Severity {
        match self {
            Self::Sh001
            | Self::Sh003
            | Self::Sh004
            | Self::Sh007
            | Self::Sh009
            | Self::Sh011
            | Self::Lq001
            | Self::Lq002
            | Self::Lq003
            | Self::Lq006
            | Self::Lq010
            | Self::V2003
            | Self::V2004
            | Self::V2005
            | Self::V2007
            | Self::V2010
            | Self::V2011 => Severity::Warning,
            Self::Sh002
            | Self::Sh005
            | Self::Sh006
            | Self::Sh008
            | Self::Lq004
            | Self::Lq005
            | Self::Lq007
            | Self::Lq008
            | Self::V2001
            | Self::V2002
            | Self::V2006
            | Self::V2008
            | Self::V2009 => Severity::Info,
            Self::Sh010 | Self::Lq009 | Self::Td001 | Self::Td002 | Self::Td003 => Severity::Hint,
        }
    }
}

/// The default configuration written as a `spec/lint.yaml` that states every default.
#[must_use]
pub fn defaults_text() -> String {
    let mut text = String::from("rules:\n");
    for rule in RuleId::ALL {
        let _ = write!(
            text,
            "  - rule: {rule}\n    enabled: {}\n    severity: {}\n",
            rule.enabled_by_default(),
            rule.default_severity()
        );
        for (_, key, kind) in OPTIONS.iter().filter(|(each, ..)| each == rule) {
            option_text(&mut text, key, kind);
        }
    }
    text
}

/// An option at its default, as the published defaults write it.
fn option_text(text: &mut String, key: &str, kind: &Kind) {
    match kind {
        Kind::Lexicon(terms) => {
            let _ = writeln!(text, "    {key}:\n      replace:");
            for term in *terms {
                let _ = writeln!(text, "        - \"{term}\"");
            }
        }
        Kind::Count { default, .. } => {
            let _ = writeln!(text, "    {key}: {default}");
        }
        Kind::Millionths(default) => {
            let threshold = super::config::millionths_text(*default);
            let _ = writeln!(text, "    {key}: {threshold}");
        }
        // No names: a list written with no value.
        Kind::Names => {
            let _ = writeln!(text, "    {key}:");
        }
        Kind::Groups(groups) => {
            let _ = writeln!(text, "    {key}:\n      replace:");
            for (name, words) in *groups {
                let _ = writeln!(text, "        - name: \"{name}\"\n          words:");
                for word in *words {
                    let _ = writeln!(text, "            - \"{word}\"");
                }
            }
        }
    }
}

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

/// `V2-006` reports a Requirement with more criteria than this.
pub(super) const MAX_CRITERIA: usize = 20;

/// `V2-006` reports a Requirement whose body holds more words than this.
pub(super) const MAX_BODY_WORDS: usize = 150;

/// LQ-005 reports a segment whose conjunctions reach this count.
pub(super) const CONJUNCTION_THRESHOLD: usize = 3;

/// LQ-008 compares names of at least this many tokens.
pub(super) const MIN_TOKENS: usize = 2;

/// LQ-008 reports two names whose similarity reaches this, in millionths.
pub(super) const SIMILARITY_MILLIONTHS: u32 = 800_000;

/// The lexicon a lexical rule reads by default.
pub(super) const fn lexicon(rule: RuleId) -> Option<&'static [&'static str]> {
    match rule {
        RuleId::Lq001 => Some(WEAK_TERMS),
        RuleId::Lq002 => Some(VAGUE_QUANTIFIERS),
        RuleId::Lq003 => Some(ESCAPE_CLAUSES),
        RuleId::Lq004 => Some(ABSOLUTES),
        RuleId::V2007 => Some(NORMATIVE_WORDS),
        _ => None,
    }
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
            | Self::Lq001
            | Self::Lq002
            | Self::Lq003
            | Self::Lq006
            | Self::V2003
            | Self::V2004
            | Self::V2005
            | Self::V2007 => Severity::Warning,
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
            | Self::V2008 => Severity::Info,
            Self::Sh010 | Self::Lq009 => Severity::Hint,
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
        let list = |text: &mut String, field: &str, terms: &[&str]| {
            let _ = writeln!(text, "    {field}:\n      replace:");
            for term in terms {
                let _ = writeln!(text, "        - \"{term}\"");
            }
        };
        if let Some(terms) = lexicon(*rule) {
            list(&mut text, "lexicon", terms);
        }
        if *rule == RuleId::Lq005 {
            list(&mut text, "modalLexicon", MODALS);
            list(&mut text, "conjunctionLexicon", CONJUNCTIONS);
            let _ = writeln!(text, "    conjunctionThreshold: {CONJUNCTION_THRESHOLD}");
        }
        if *rule == RuleId::V2006 {
            let _ = writeln!(text, "    maxCriteria: {MAX_CRITERIA}");
            let _ = writeln!(text, "    maxBodyWords: {MAX_BODY_WORDS}");
        }
        if *rule == RuleId::Lq008 {
            let _ = writeln!(text, "    minTokens: {MIN_TOKENS}");
            let threshold = super::config::millionths_text(SIMILARITY_MILLIONTHS);
            let _ = writeln!(text, "    threshold: {threshold}");
        }
    }
    text
}

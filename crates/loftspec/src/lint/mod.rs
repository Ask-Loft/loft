//! The linter (CMP-7, F-19..F-23): the structural rules `SH-001`..`SH-010`, the language
//! rules `LQ-001`..`LQ-009`, the v2 rules `V2-001`..`V2-004` and `V2-009`, and the rules of the
//! writing craft `V2-005`..`V2-008`, each finding a typed record in a stable order. The
//! configuration is `spec/lint.yaml`, part of the tree the linter is given; no flag, environment
//! variable, clock or locale changes a finding (RQ-106). The linter changes nothing in the
//! document (CON-9).

mod config;
mod context;
mod craft;
mod defaults;
mod language;
mod lexicon;
mod markdown;
mod similarity;
mod structural;
mod text;
mod v2;

use std::cmp::Ordering;
use std::fmt;

pub use config::{LintConfig, Similarity};
pub use defaults::defaults_text;

use crate::codec::{Site, Sites, write_sites};
use crate::document::Document;
use crate::identity::Key;
use crate::wire::wire_names;

use self::context::Context;

/// A rule of the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleId {
    Sh001,
    Sh002,
    Sh003,
    Sh004,
    Sh005,
    Sh006,
    Sh007,
    Sh008,
    Sh009,
    Sh010,
    Lq001,
    Lq002,
    Lq003,
    Lq004,
    Lq005,
    Lq006,
    Lq007,
    Lq008,
    Lq009,
    V2001,
    V2002,
    V2003,
    V2004,
    V2005,
    V2006,
    V2007,
    V2008,
    V2009,
}

wire_names!(RuleId {
    Sh001 => "SH-001",
    Sh002 => "SH-002",
    Sh003 => "SH-003",
    Sh004 => "SH-004",
    Sh005 => "SH-005",
    Sh006 => "SH-006",
    Sh007 => "SH-007",
    Sh008 => "SH-008",
    Sh009 => "SH-009",
    Sh010 => "SH-010",
    Lq001 => "LQ-001",
    Lq002 => "LQ-002",
    Lq003 => "LQ-003",
    Lq004 => "LQ-004",
    Lq005 => "LQ-005",
    Lq006 => "LQ-006",
    Lq007 => "LQ-007",
    Lq008 => "LQ-008",
    Lq009 => "LQ-009",
    V2001 => "V2-001",
    V2002 => "V2-002",
    V2003 => "V2-003",
    V2004 => "V2-004",
    V2005 => "V2-005",
    V2006 => "V2-006",
    V2007 => "V2-007",
    V2008 => "V2-008",
    V2009 => "V2-009",
});

impl fmt::Display for RuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// How much a finding matters, from least to most. Ordered so that a finding is "at or above" a
/// severity when it compares greater or equal (RQ-139).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Hint,
    Info,
    Warning,
}

wire_names!(Severity {
    Hint => "hint",
    Info => "info",
    Warning => "warning",
});

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// An element a finding names, and where it is written when the tree says so (AC-417, AC-418,
/// AC-419).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    pub key: Key,
    pub site: Option<Site>,
}

/// One finding (RQ-103).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: RuleId,
    pub severity: Severity,
    /// The elements the finding names, in the order the rule names them.
    pub subjects: Vec<Subject>,
    /// The term, tag, clause or reference the finding is about, where it has one.
    pub term: Option<String>,
    /// The line of the body the finding is in, counted from 1, where it is in a body (AC-371).
    pub body_line: Option<usize>,
    pub message: String,
}

impl Finding {
    /// The order of a report: file path, line, column, rule, then the keys the finding names
    /// (AC-420); then the body line and the term, so that no two findings tie.
    fn order(&self, other: &Self) -> Ordering {
        let site = |finding: &Self| finding.subjects.first().and_then(|s| s.site.clone());
        let keys = |finding: &Self| finding.subjects.iter().map(|s| s.key).collect::<Vec<_>>();
        site(self)
            .cmp(&site(other))
            .then(self.rule.cmp(&other.rule))
            .then_with(|| keys(self).cmp(&keys(other)))
            .then(self.body_line.cmp(&other.body_line))
            .then_with(|| self.term.cmp(&other.term))
    }
}

impl fmt::Display for Finding {
    /// `<path>:<line>:<column>: <severity> <rule>: <message>`, every site the finding names
    /// first (RQ-158), then the term and the body line where the finding has them.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sites = self
            .subjects
            .iter()
            .map(|subject| subject.site.as_ref().ok_or(subject.key));
        write_sites(formatter, sites)?;
        write!(
            formatter,
            "{} {}: {}",
            self.severity, self.rule, self.message
        )?;
        if let Some(term) = &self.term {
            write!(formatter, " `{term}`")?;
        }
        if let Some(line) = self.body_line {
            write!(formatter, " (body line {line})")?;
        }
        Ok(())
    }
}

/// A finding before its subjects are placed: the rule, the keys it names and what it says.
pub(crate) struct Raw {
    pub rule: RuleId,
    pub keys: Vec<Key>,
    pub term: Option<String>,
    pub body_line: Option<usize>,
    pub message: String,
}

impl Raw {
    pub(crate) fn new(rule: RuleId, keys: Vec<Key>, message: impl Into<String>) -> Self {
        Self {
            rule,
            keys,
            term: None,
            body_line: None,
            message: message.into(),
        }
    }

    pub(crate) fn term(mut self, term: impl Into<String>) -> Self {
        self.term = Some(term.into());
        self
    }

    pub(crate) const fn line(mut self, body_line: Option<usize>) -> Self {
        self.body_line = body_line;
        self
    }
}

/// Lints a document with a configuration, placing each finding at the sites the tree gives.
/// Disabled rules report nothing; suppressed findings are dropped (AC-675); the findings come
/// back in report order (RQ-104).
#[must_use]
pub fn lint(document: &Document, sites: &Sites, config: &LintConfig) -> Vec<Finding> {
    let context = Context::new(document);
    let mut raw = Vec::new();
    structural::check(&context, &mut raw);
    language::check(&context, config, &mut raw);
    v2::check(&context, &mut raw);
    craft::check(&context, config, &mut raw);
    let mut findings: Vec<Finding> = raw
        .into_iter()
        .filter(|finding| config.enabled(finding.rule))
        .filter(|finding| !config.suppresses(finding.rule, &finding.keys))
        .map(|raw| Finding {
            rule: raw.rule,
            severity: config.severity(raw.rule),
            subjects: raw
                .keys
                .iter()
                .map(|key| Subject {
                    key: *key,
                    site: sites.get(*key).cloned(),
                })
                .collect(),
            term: raw.term,
            body_line: raw.body_line,
            message: raw.message,
        })
        .collect();
    findings.sort_by(Finding::order);
    findings
}

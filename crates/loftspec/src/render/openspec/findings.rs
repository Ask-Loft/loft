//! What the `OpenSpec` renderer reports: each place where `OpenSpec`'s validator would reject or
//! warn about the rendered specs, and the content the projection leaves out. Every finding has
//! the severity info; none prevents the render.

use std::fmt;

use crate::identity::Key;
use crate::lint::Severity;
use crate::wire::wire_names;

/// The rule a finding follows; its wire name is its code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    NoModal,
    NoScenario,
    ShortPurpose,
    LongRequirement,
    LeftOut,
    DuplicateName,
}

wire_names!(Rule {
    NoModal => "OR-001",
    NoScenario => "OR-002",
    ShortPurpose => "OR-003",
    LongRequirement => "OR-004",
    LeftOut => "OR-005",
    DuplicateName => "OR-006",
});

impl Rule {
    /// What a finding of the rule says, before its detail.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::NoModal => {
                "the requirement text holds neither SHALL nor MUST; OpenSpec rejects it"
            }
            Self::NoScenario => "the requirement has no scenario with text; OpenSpec rejects it",
            Self::ShortPurpose => {
                "the Purpose is shorter than 50 characters; OpenSpec warns about it"
            }
            Self::LongRequirement => {
                "the requirement text is longer than 500 characters; OpenSpec warns about it"
            }
            Self::LeftOut => "content OpenSpec has no place for was left out",
            Self::DuplicateName => {
                "an earlier requirement of the spec has the same name; OpenSpec rejects it"
            }
        }
    }
}

/// One finding: its rule, the rendered file and the 1-based line of the heading it concerns
/// where it has one, the key of the node it concerns where there is one, and a detail such as
/// a count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: Rule,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub key: Option<Key>,
    pub detail: Option<String>,
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(formatter, "{file}:")?;
            if let Some(line) = self.line {
                write!(formatter, "{line}:")?;
            }
            formatter.write_str(" ")?;
        }
        write!(
            formatter,
            "{} {}: {}",
            Severity::Info.as_str(),
            self.rule.as_str(),
            self.rule.message()
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

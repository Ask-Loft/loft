//! What the converter reports about the project it converts. Every finding has a code and the
//! severity info; none stops the conversion.

use std::fmt;

use crate::identity::Key;

/// The rule a finding follows, by its code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    FrontMatter,
    PlaceholderPurpose,
    ExtraSection,
    StrayRequirementsText,
    NonScenarioChild,
    DeltaSection,
    NoModal,
    ModalRewritten,
    Metadata,
    DuplicateRequirement,
    ActiveChanges,
    ArchivedChanges,
    ConfigContext,
    Store,
}

impl Rule {
    pub const ALL: [Self; 14] = [
        Self::FrontMatter,
        Self::PlaceholderPurpose,
        Self::ExtraSection,
        Self::StrayRequirementsText,
        Self::NonScenarioChild,
        Self::DeltaSection,
        Self::NoModal,
        Self::ModalRewritten,
        Self::Metadata,
        Self::DuplicateRequirement,
        Self::ActiveChanges,
        Self::ArchivedChanges,
        Self::ConfigContext,
        Self::Store,
    ];

    /// The code the finding carries.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::FrontMatter => "OS-001",
            Self::PlaceholderPurpose => "OS-002",
            Self::ExtraSection => "OS-003",
            Self::StrayRequirementsText => "OS-004",
            Self::NonScenarioChild => "OS-005",
            Self::DeltaSection => "OS-006",
            Self::NoModal => "OS-007",
            Self::ModalRewritten => "OS-008",
            Self::Metadata => "OS-009",
            Self::DuplicateRequirement => "OS-010",
            Self::ActiveChanges => "OS-011",
            Self::ArchivedChanges => "OS-012",
            Self::ConfigContext => "OS-013",
            Self::Store => "OS-014",
        }
    }

    /// What a finding of the rule says, before its detail.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::FrontMatter => "front matter was skipped",
            Self::PlaceholderPurpose => {
                "the Purpose is OpenSpec's archive placeholder; the Feature has no body"
            }
            Self::ExtraSection => "a section other than Purpose and Requirements became a Section",
            Self::StrayRequirementsText => {
                "text of Requirements outside every requirement became a Section"
            }
            Self::NonScenarioChild => {
                "a child of a requirement that is not a scenario became a Section"
            }
            Self::DeltaSection => "a delta section in a main spec became a Section",
            Self::NoModal => "the requirement holds neither SHALL nor MUST",
            Self::ModalRewritten => "statements whose SHALL or MUST was written as shall",
            Self::Metadata => "a metadata line was kept in the Requirement's body",
            Self::DuplicateRequirement => "a second requirement has the same name",
            Self::ActiveChanges => "active changes were not converted",
            Self::ArchivedChanges => "archived changes were not converted",
            Self::ConfigContext => "the context of config.yaml was not converted",
            Self::Store => "config.yaml names a store; the main specs may be kept elsewhere",
        }
    }
}

/// One finding: its rule, the source file and 1-based line it concerns where it has one, the
/// key of the node it concerns where there is one, and a detail such as a count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: Rule,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub key: Option<Key>,
    pub detail: Option<String>,
}

impl Finding {
    pub(super) const fn at(rule: Rule, file: String, line: usize, key: Option<Key>) -> Self {
        Self {
            rule,
            file: Some(file),
            line: Some(line),
            key,
            detail: None,
        }
    }

    pub(super) const fn about(rule: Rule, detail: String) -> Self {
        Self {
            rule,
            file: None,
            line: None,
            key: None,
            detail: Some(detail),
        }
    }
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
            "info {}: {}",
            self.rule.code(),
            self.rule.message()
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

//! What the converter reports about the project it converts. Every finding has a code and the
//! severity info; none stops the conversion.

use crate::report::{InfoFinding, InfoRule};
use crate::wire::wire_names;

/// The rule a finding follows; its wire name is its code.
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
    DuplicateRequirement,
    ActiveChanges,
    ArchivedChanges,
    ConfigContext,
    Store,
}

wire_names!(Rule {
    FrontMatter => "OS-001",
    PlaceholderPurpose => "OS-002",
    ExtraSection => "OS-003",
    StrayRequirementsText => "OS-004",
    NonScenarioChild => "OS-005",
    DeltaSection => "OS-006",
    NoModal => "OS-007",
    ModalRewritten => "OS-008",
    DuplicateRequirement => "OS-010",
    ActiveChanges => "OS-011",
    ArchivedChanges => "OS-012",
    ConfigContext => "OS-013",
    Store => "OS-014",
});

impl Rule {
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
            Self::DuplicateRequirement => "a second requirement has the same name",
            Self::ActiveChanges => "active changes were not converted",
            Self::ArchivedChanges => "archived changes were not converted",
            Self::ConfigContext => "the context of config.yaml was not converted",
            Self::Store => "config.yaml names a store; the main specs may be kept elsewhere",
        }
    }
}

impl InfoRule for Rule {
    fn code(self) -> &'static str {
        self.as_str()
    }

    fn message(self) -> &'static str {
        Self::message(self)
    }
}

/// A finding of one of the rules.
pub type Finding = InfoFinding<Rule>;

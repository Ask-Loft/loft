//! What the `OpenSpec` renderer reports: each place where `OpenSpec`'s validator would reject or
//! warn about the rendered specs, and the content the projection leaves out. Every finding has
//! the severity info; none prevents the render.

use crate::report::{InfoFinding, InfoRule};
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
    ReadsBack,
    UnreadableExt,
    PlaceholderPurpose,
    NoRequirement,
    SectionAsRequirement,
    DeltaSection,
}

wire_names!(Rule {
    NoModal => "OR-001",
    NoScenario => "OR-002",
    ShortPurpose => "OR-003",
    LongRequirement => "OR-004",
    LeftOut => "OR-005",
    DuplicateName => "OR-006",
    ReadsBack => "OR-007",
    UnreadableExt => "OR-008",
    PlaceholderPurpose => "OR-009",
    NoRequirement => "OR-010",
    SectionAsRequirement => "OR-011",
    DeltaSection => "OR-012",
});

impl Rule {
    /// What a finding of the rule says, before its detail.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::NoModal => {
                "the requirement text holds neither SHALL nor MUST; OpenSpec warns about it"
            }
            Self::NoScenario => {
                "the requirement has no scenario with text; OpenSpec reports an error"
            }
            Self::ShortPurpose => {
                "the Purpose is shorter than 50 characters; OpenSpec warns about it, or reports an \
                 error when it is empty"
            }
            Self::LongRequirement => {
                "the requirement text is longer than 500 characters; OpenSpec warns about it"
            }
            Self::LeftOut => "content OpenSpec has no place for was left out",
            Self::DuplicateName => {
                "an earlier requirement of the spec has the same name; OpenSpec reports an error"
            }
            Self::ReadsBack => {
                "content a second import would read differently was written as it is"
            }
            Self::UnreadableExt => "an ext.openspec value could not be read and was left out",
            Self::PlaceholderPurpose => "the Purpose is a placeholder; OpenSpec warns about it",
            Self::NoRequirement => "the spec has no requirement; OpenSpec reports an error",
            Self::SectionAsRequirement => {
                "a Section is written as a requirement heading outside Requirements; OpenSpec \
                 reports an error"
            }
            Self::DeltaSection => {
                "a Section is written as a delta section heading; OpenSpec reports an error"
            }
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

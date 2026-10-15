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

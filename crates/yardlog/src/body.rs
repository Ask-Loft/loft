//! What one version of a block holds, for each of the eight kinds.

use crate::act::Time;
use crate::datum::Datum;
use crate::keys::{BlockKey, Key};

macro_rules! words {
    ($(#[$doc:meta])* $name:ident { $($variant:ident = $word:literal),* $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant),*];

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $word),*
                }
            }

            #[must_use]
            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($word => Some(Self::$variant),)*
                    _ => None,
                }
            }

            /// The words this field takes, as a refusal lists them.
            #[must_use]
            pub fn words() -> String {
                [$(concat!("`", $word, "`")),*].join(", ")
            }
        }
    };
}

words! {
    /// A block's kind.
    Kind {
        Stage = "stage",
        Note = "note",
        Question = "question",
        Approval = "approval",
        Decision = "decision",
        Task = "task",
        Plan = "plan",
        Loft = "loft",
    }
}

words! {
    /// A stage's status.
    Status { Planned = "planned", Active = "active", Done = "done", Dropped = "dropped" }
}

words! {
    /// A task's state.
    TaskState { Open = "open", Active = "active", Done = "done", Dropped = "dropped" }
}

words! {
    /// A note's tone.
    Tone { Info = "info", Success = "success", Warning = "warning", Danger = "danger" }
}

words! {
    /// The report of `loft`'s that a `loft` block records.
    View {
        SpecChange = "spec-change",
        Decision = "decision",
        Trace = "trace",
        Findings = "findings",
        Publish = "publish",
    }
}

words! {
    /// The person's decision of an approval or a plan.
    Verdict { Approved = "approved", Rejected = "rejected" }
}

impl Kind {
    /// Whether the person answers or decides blocks of this kind.
    #[must_use]
    pub const fn is_asked(self) -> bool {
        matches!(self, Self::Question | Self::Approval | Self::Plan)
    }
}

/// One version of a block: its kind's fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Stage(Stage),
    Note(Note),
    Question(Question),
    Approval(Approval),
    Decision(Decision),
    Task(Task),
    Plan(Plan),
    Loft(Box<Loft>),
}

impl Body {
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Stage(_) => Kind::Stage,
            Self::Note(_) => Kind::Note,
            Self::Question(_) => Kind::Question,
            Self::Approval(_) => Kind::Approval,
            Self::Decision(_) => Kind::Decision,
            Self::Task(_) => Kind::Task,
            Self::Plan(_) => Kind::Plan,
            Self::Loft(_) => Kind::Loft,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub title: String,
    pub goal: Option<String>,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub title: Option<String>,
    pub markdown: String,
    pub tone: Option<Tone>,
    pub folded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub prompt: String,
    pub header: String,
    pub context: Option<String>,
    pub options: Vec<Choice>,
    /// Whether more than one option may be chosen.
    pub multiple: bool,
    /// Whether the person may answer in their own words beside the options.
    pub own_words: bool,
}

/// One option of a question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub label: String,
    pub detail: Option<String>,
    pub recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    pub title: String,
    pub markdown: String,
}

/// A decision's fields; its number belongs to the block, which keeps it through every version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub title: String,
    pub consequence: String,
    /// The question it settles.
    pub question: Option<BlockKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub title: String,
    pub detail: Option<String>,
    pub state: TaskState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub title: String,
    pub markdown: String,
    /// The file the plan came from.
    pub path: Option<String>,
    /// Whether the agent presented it in plan mode.
    pub plan_mode: bool,
}

/// A report of `loft`'s, as the block recorded it, with what the agent asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loft {
    pub view: View,
    pub note: Option<String>,
    /// The base revision a `spec-change`, `trace` or `findings` view compares with.
    pub base: Option<String>,
    /// The decision a `decision` view records, by its slug, key or label.
    pub decision: Option<String>,
    /// The agent's reasons for the findings a `findings` view leaves in place.
    pub reasons: Vec<Reason>,
    pub recorded: Recorded,
    pub report: Datum,
}

/// Why the agent leaves a finding in place, by its rule and the key of the node it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    pub rule: String,
    pub key: Key,
    pub reason: String,
}

/// Where and when a report was computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub commit: Option<String>,
    pub branch: Option<String>,
    /// Whether the working tree held changes not committed.
    pub dirty: bool,
    /// The version of the `loft` that computed it.
    pub loft: String,
    pub at: Time,
}

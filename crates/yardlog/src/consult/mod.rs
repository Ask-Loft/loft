//! What a session's agents consulted in one branch's Yard Log, as their tool calls and model
//! requests add to it (the Feature "Count what the agents consulted"): six counts of reads and
//! edits, in all, by the stage a call counts toward and within it by agent, and under no stage;
//! for each stage and agent, when the specification was first consulted, when the code was
//! first edited and the edits of code before that consultation; and the context tokens each
//! class of read is estimated to have brought in. A session's consultation file in Yard Log's
//! folder holds them, `consultation/<session id>.yaml`, in the format `yardconsult/1`.

mod estimate;
mod file;
mod json;
mod sums;

use std::collections::BTreeMap;

use crate::Time;
use crate::totals::Who;

pub use estimate::{Attributed, Estimate, Live};
pub use json::tallies;
pub use sums::{SessionSum, Sums};

/// What a path a tool call reads or edits is: the specification, the code, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    Spec,
    Code,
    Other,
}

impl Class {
    /// The class as an event names it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Spec => "spec",
            Self::Code => "code",
            Self::Other => "other",
        }
    }

    /// The class an event names, where it names one.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        [Self::Spec, Self::Code, Self::Other]
            .into_iter()
            .find(|class| class.name() == name)
    }
}

/// The reads and edits counted: of the specification, of code and of other files, and the runs
/// of `loft` that read the specification.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Six {
    pub spec_reads: u64,
    pub code_reads: u64,
    pub other_reads: u64,
    pub loft_reads: u64,
    pub spec_edits: u64,
    pub code_edits: u64,
}

impl Six {
    /// The names the counts are written under, in the order of [`Six::values`].
    const NAMES: [&str; 6] = [
        "specReads",
        "codeReads",
        "otherReads",
        "loftReads",
        "specEdits",
        "codeEdits",
    ];

    const fn values(self) -> [u64; 6] {
        [
            self.spec_reads,
            self.code_reads,
            self.other_reads,
            self.loft_reads,
            self.spec_edits,
            self.code_edits,
        ]
    }

    const fn from_values(values: [u64; 6]) -> Self {
        let [
            spec_reads,
            code_reads,
            other_reads,
            loft_reads,
            spec_edits,
            code_edits,
        ] = values;
        Self {
            spec_reads,
            code_reads,
            other_reads,
            loft_reads,
            spec_edits,
            code_edits,
        }
    }

    /// The counts of `call`.
    fn of(call: &Call<'_>) -> Self {
        let has = |classes: &[Class], class| u64::from(classes.contains(&class));
        Self {
            spec_reads: has(call.reads, Class::Spec),
            code_reads: has(call.reads, Class::Code),
            other_reads: has(call.reads, Class::Other),
            loft_reads: u64::from(call.loft_read),
            spec_edits: has(call.edits, Class::Spec),
            code_edits: has(call.edits, Class::Code),
        }
    }

    pub fn add(&mut self, other: Self) {
        let mut values = self.values();
        for (value, more) in values.iter_mut().zip(other.values()) {
            *value += more;
        }
        *self = Self::from_values(values);
    }

    /// The consultations of the specification: its reads and the `loft` reads.
    #[must_use]
    pub const fn spec_consultations(self) -> u64 {
        self.spec_reads + self.loft_reads
    }
}

/// When the specification was first consulted and the code first edited, and the edits of code
/// made before that consultation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Order {
    pub first_spec: Option<Time>,
    pub first_code_edit: Option<Time>,
    pub code_edits_before_spec: u64,
}

impl Order {
    /// Notes a call at `at`. A call that both consults the specification and edits code, as
    /// `loft check && apply_patch` does, consults it first: its edit comes after.
    fn note(&mut self, at: Time, consulted: bool, edited_code: bool) {
        if consulted {
            self.first_spec.get_or_insert(at);
        }
        if edited_code {
            self.first_code_edit.get_or_insert(at);
            if self.first_spec.is_none() {
                self.code_edits_before_spec += 1;
            }
        }
    }

    /// Whether the specification was consulted before the code was first edited, where the code
    /// was edited at all.
    #[must_use]
    pub fn spec_first(self) -> Option<bool> {
        self.first_code_edit
            .map(|_| self.code_edits_before_spec == 0)
    }
}

/// What counted toward one scope: a stage, an agent within one, or none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Scope {
    pub counts: Six,
    pub order: Order,
    pub estimate: Estimate,
}

impl Scope {
    fn count(&mut self, at: Time, counts: Six) {
        self.counts.add(counts);
        let consulted = counts.spec_consultations() > 0;
        self.order.note(at, consulted, counts.code_edits > 0);
    }
}

/// What counted toward one stage, or toward none: in all, and by agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stage {
    pub all: Scope,
    pub agents: BTreeMap<Who, Scope>,
}

impl Stage {
    /// The stage's scope in all and its agent `who`'s.
    fn scopes(&mut self, who: &Who) -> [&mut Scope; 2] {
        let agent = self.agents.entry(who.clone()).or_default();
        [&mut self.all, agent]
    }
}

/// A tool call's start, as the consultation counts it.
#[derive(Debug, Clone)]
pub struct Call<'a> {
    /// When it started.
    pub at: Time,
    pub who: &'a Who,
    /// The call's id, by which its end is known.
    pub id: &'a str,
    pub reads: &'a [Class],
    pub edits: &'a [Class],
    /// Whether it ran a `loft` subcommand that reads the specification.
    pub loft_read: bool,
}

/// The `loft` subcommands that read the specification.
pub const LOFT_READS: [&str; 8] = [
    "check",
    "find",
    "terms",
    "reconcile",
    "verify",
    "status",
    "review",
    "render",
];

/// Whether any of the `loft` subcommands `names` reads the specification.
pub fn reads_spec<'a>(names: impl IntoIterator<Item = &'a str>) -> bool {
    names.into_iter().any(|name| LOFT_READS.contains(&name))
}

/// A session's consultation counts in one branch's Yard Log.
#[derive(Debug, Clone, Default)]
pub struct Consultation {
    pub session: String,
    pub host: String,
    pub first_started: Option<Time>,
    pub last_started: Option<Time>,
    /// When the last event it took in happened, by which a merge takes the later copy.
    pub last_ended: Option<Time>,
    pub counts: Six,
    pub estimate: Estimate,
    pub stages: BTreeMap<String, Stage>,
    pub no_stage: Option<Stage>,
    /// Whether the estimate left out a request that lacked a count.
    pub partial: bool,
}

impl Consultation {
    #[must_use]
    pub fn new(session: &str, host: &str) -> Self {
        Self {
            session: session.to_owned(),
            host: host.to_owned(),
            ..Self::default()
        }
    }

    /// Whether any call was counted, so that the file is written.
    #[must_use]
    pub fn counted(&self) -> bool {
        self.counts != Six::default()
    }

    /// What counted toward `stage`, or toward none.
    fn stage_mut(&mut self, stage: Option<&str>) -> &mut Stage {
        match stage {
            Some(stage) => self.stages.entry(stage.to_owned()).or_default(),
            None => self.no_stage.get_or_insert_with(Stage::default),
        }
    }

    /// Counts the start of `call` toward `stage`; says whether it counted.
    pub fn start(&mut self, call: &Call<'_>, stage: Option<&str>) -> bool {
        let counts = Six::of(call);
        if counts == Six::default() {
            return false;
        }
        self.first_started.get_or_insert(call.at);
        self.last_started = self.last_started.max(Some(call.at));
        self.last_ended = self.last_ended.max(Some(call.at));
        self.counts.add(counts);
        for scope in self.stage_mut(stage).scopes(call.who) {
            scope.count(call.at, counts);
        }
        true
    }

    /// Keeps what the request of `who` that ended `at`, counted toward `stage`, attributed;
    /// says whether the file's text changed.
    pub fn request(
        &mut self,
        who: &Who,
        at: Time,
        attributed: Attributed,
        stage: Option<&str>,
    ) -> bool {
        let Attributed {
            estimate: attributed,
            partial,
        } = attributed;
        let any = attributed != Estimate::default();
        let changed = any || (partial && !self.partial);
        self.partial |= partial;
        if any {
            self.estimate.add(attributed);
            for scope in self.stage_mut(stage).scopes(who) {
                scope.estimate.add(attributed);
            }
        }
        if changed {
            self.last_ended = self.last_ended.max(Some(at));
        }
        changed
    }
}

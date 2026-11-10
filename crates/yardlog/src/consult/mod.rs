//! What a session's agents consulted in one branch's Yard Log, as their tool calls and model
//! requests add to it (the Feature "Count what the agents consulted"): six counts of reads and
//! edits, in all, by the stage a call counts toward and within it by agent, and under no stage;
//! for each stage and agent, when the specification was first consulted, when the code was
//! first edited and the edits of code before that consultation; and the context tokens each
//! class of read is estimated to have brought in. A session's consultation file in Yard Log's
//! folder holds them, `consultation/<session id>.yaml`, in the format `yardconsult/1`.

mod estimate;
mod file;
mod sums;

use std::collections::BTreeMap;

use crate::Time;
use crate::totals::Who;
use crate::usage::Counts;

pub use estimate::Estimate;
pub use file::FORMAT;
pub use sums::sums;

use estimate::Live;

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
        match name {
            "spec" => Some(Self::Spec),
            "code" => Some(Self::Code),
            "other" => Some(Self::Other),
            _ => None,
        }
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
        self.spec_reads += other.spec_reads;
        self.code_reads += other.code_reads;
        self.other_reads += other.other_reads;
        self.loft_reads += other.loft_reads;
        self.spec_edits += other.spec_edits;
        self.code_edits += other.code_edits;
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
    fn note(&mut self, at: Time, consulted: bool, edited_code: bool) {
        if edited_code {
            self.first_code_edit.get_or_insert(at);
            if self.first_spec.is_none() {
                self.code_edits_before_spec += 1;
            }
        }
        if consulted {
            self.first_spec.get_or_insert(at);
        }
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
    fn count(&mut self, call: &Call<'_>, counts: Six) {
        self.counts.add(counts);
        let consulted = counts.spec_consultations() > 0;
        self.order.note(call.at, consulted, counts.code_edits > 0);
    }
}

/// What counted toward one stage: in all, and by agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stage {
    pub all: Scope,
    pub agents: BTreeMap<Who, Scope>,
}

/// A tool call's start, as the consultation counts it.
#[derive(Debug, Clone)]
pub struct Call<'a> {
    /// The agent host the session runs in.
    pub host: &'a str,
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
    pub all: Scope,
    pub stages: BTreeMap<String, Stage>,
    pub no_stage: Option<Stage>,
    /// Whether the estimate left out a request that lacked a count.
    pub partial: bool,
    /// What the estimate needs between events, and no file holds.
    live: Live,
}

impl Consultation {
    #[must_use]
    pub fn new(session: &str) -> Self {
        Self {
            session: session.to_owned(),
            ..Self::default()
        }
    }

    /// Whether any call was counted, so that the file is written.
    #[must_use]
    pub const fn counted(&self) -> bool {
        self.first_started.is_some()
    }

    /// What counted toward `stage`, or toward none.
    fn stage_mut(&mut self, stage: Option<&str>) -> &mut Stage {
        match stage {
            Some(stage) => self.stages.entry(stage.to_owned()).or_default(),
            None => self.no_stage.get_or_insert_with(Stage::default),
        }
    }

    /// Counts the start of `call` toward `stage`; says whether it counted. Every call is noted
    /// for the estimate, counted or not.
    pub fn start(&mut self, call: &Call<'_>, stage: Option<&str>) -> bool {
        self.live.started(call);
        let counts = Six::of(call);
        if counts == Six::default() {
            return false;
        }
        if self.host.is_empty() {
            call.host.clone_into(&mut self.host);
        }
        self.first_started.get_or_insert(call.at);
        self.last_started = Some(self.last_started.map_or(call.at, |last| last.max(call.at)));
        self.last_ended = Some(self.last_ended.map_or(call.at, |last| last.max(call.at)));
        self.all.counts.add(counts);
        let counted = self.stage_mut(stage);
        counted.all.count(call, counts);
        let agent = counted.agents.entry(call.who.clone()).or_default();
        agent.count(call, counts);
        true
    }

    /// Notes the end of the call `id` of `who`, whose result the agent's next request reads.
    pub fn end(&mut self, who: &Who, id: &str) {
        self.live.ended(who, id);
    }

    /// Attributes the request of `who` that ended `at` with `tokens`, counted toward `stage`;
    /// says whether the counts changed.
    pub fn request(
        &mut self,
        who: &Who,
        at: Time,
        tokens: Option<Counts>,
        stage: Option<&str>,
    ) -> bool {
        let (attributed, partial) = self.live.requested(who, tokens);
        let changed = (partial && !self.partial) || attributed != Estimate::default();
        self.partial |= partial;
        if attributed != Estimate::default() {
            self.all.estimate.add(attributed);
            let counted = self.stage_mut(stage);
            counted.all.estimate.add(attributed);
            let agent = counted.agents.entry(who.clone()).or_default();
            agent.estimate.add(attributed);
        }
        if changed {
            self.last_ended = Some(self.last_ended.map_or(at, |last| last.max(at)));
        }
        changed
    }
}

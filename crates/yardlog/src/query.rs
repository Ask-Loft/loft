//! What follows from the records without being written: a block's current version, the answer or
//! outcome that counts, when a stage or task was listed, started and ended, who owns a stage,
//! and a submission's state. Records are ordered by their Act's time, and by key between two of
//! one time, so two copies that hold the same records derive the same.

use crate::act::{Act, Actor, Agent, Time};
use crate::body::{Body, Status, TaskState};
use crate::keys::Key;
use crate::log::{Arrival, Block, Happened, Response, Submission, Version, YardLog};

/// The revision at which a record reached this copy: where a merge brought it, the revision of
/// the settling, and otherwise the revision of its Act.
#[must_use]
pub fn reached(act: &Act, arrived: Option<Arrival>) -> u64 {
    match arrived {
        Some(Arrival::At(revision)) => revision,
        _ => act.revision,
    }
}

fn order(act: &Act, key: Key) -> (Time, Key) {
    (act.at, key)
}

/// When a stage or task was listed, and its work started and ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Times {
    pub listed: Option<Time>,
    pub started: Option<Time>,
    pub ended: Option<Time>,
}

/// Where a stage's or task's work stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Progress {
    Waiting,
    Under,
    Finished(u8),
}

fn progress(body: &Body) -> Option<Progress> {
    Some(match body {
        Body::Stage(stage) => match stage.status {
            Status::Planned => Progress::Waiting,
            Status::Active => Progress::Under,
            Status::Done => Progress::Finished(0),
            Status::Dropped => Progress::Finished(1),
        },
        Body::Task(task) => match task.state {
            TaskState::Open => Progress::Waiting,
            TaskState::Active => Progress::Under,
            TaskState::Done => Progress::Finished(0),
            TaskState::Dropped => Progress::Finished(1),
        },
        _ => return None,
    })
}

impl Block {
    /// The versions in the order of their Acts.
    #[must_use]
    pub fn ordered(&self) -> Vec<&Version> {
        let mut versions: Vec<&Version> = self.versions.iter().collect();
        versions.sort_by_key(|version| order(&version.act, version.key.key()));
        versions
    }

    /// The latest version by its Act.
    #[must_use]
    pub fn current(&self) -> &Version {
        self.versions
            .iter()
            .max_by_key(|version| order(&version.act, version.key.key()))
            .expect("a block holds at least one version")
    }

    /// The version number of `version`: its place among the block's versions, from 1.
    #[must_use]
    pub fn number_of(&self, version: &Version) -> usize {
        self.ordered()
            .iter()
            .position(|other| other.key == version.key)
            .map_or(0, |at| at + 1)
    }

    /// The responses in the order of their Acts.
    #[must_use]
    pub fn ordered_responses(&self) -> Vec<&Response> {
        let mut responses: Vec<&Response> = self.responses.iter().collect();
        responses.sort_by_key(|response| order(&response.act, response.key.key()));
        responses
    }

    /// The answer or outcome that counts: the first given after the block was last sent, and,
    /// for a plan whose Markdown was rewritten after it was decided, after the rewrite.
    #[must_use]
    pub fn counted(&self, log: &YardLog) -> Option<&Response> {
        let reset = self.rewritten_at().max(self.sent_at(log));
        self.ordered_responses()
            .into_iter()
            .find(|response| reset.is_none_or(|at| order(&response.act, response.key.key()) > at))
    }

    /// The send of the latest submission that holds the block.
    fn sent_at(&self, log: &YardLog) -> Option<(Time, Key)> {
        log.root
            .submissions
            .iter()
            .filter(|submission| submission.blocks.contains(&self.key))
            .filter_map(|submission| {
                let sent = submission
                    .changes
                    .iter()
                    .find(|c| c.what == Happened::Sent)?;
                Some(order(&sent.act, submission.key.key()))
            })
            .max()
    }

    /// The Act of the latest version of a plan whose Markdown differs from the one before it.
    fn rewritten_at(&self) -> Option<(Time, Key)> {
        let versions = self.ordered();
        versions
            .windows(2)
            .filter(|pair| match (&pair[0].body, &pair[1].body) {
                (Body::Plan(before), Body::Plan(after)) => before.markdown != after.markdown,
                _ => false,
            })
            .map(|pair| order(&pair[1].act, pair[1].key.key()))
            .next_back()
    }

    /// When a stage or task was listed, and its work started and ended; nothing for other
    /// kinds.
    #[must_use]
    pub fn times(&self) -> Times {
        let mut times = Times::default();
        let mut before = None;
        for version in self.ordered() {
            let Some(now) = progress(&version.body) else {
                return Times::default();
            };
            times.listed.get_or_insert(version.act.at);
            if now == Progress::Under {
                times.started.get_or_insert(version.act.at);
            }
            match now {
                Progress::Finished(_) if before != Some(now) => times.ended = Some(version.act.at),
                Progress::Finished(_) => {}
                _ => times.ended = None,
            }
            before = Some(now);
        }
        times
    }

    /// The agent that last made a stage `active`.
    #[must_use]
    pub fn owner(&self) -> Option<&Agent> {
        self.ordered()
            .into_iter()
            .rev()
            .find(|version| progress(&version.body) == Some(Progress::Under))
            .and_then(|version| version.act.actor.agent())
    }

    /// Whether a stage is `active` now.
    #[must_use]
    pub fn is_active(&self) -> bool {
        progress(&self.current().body) == Some(Progress::Under)
    }

    /// The latest revision at which a version, answer or outcome of this block reached this
    /// copy.
    #[must_use]
    pub fn reached(&self) -> u64 {
        let versions = self.versions.iter().map(|v| reached(&v.act, v.arrived));
        let responses = self.responses.iter().map(|r| reached(&r.act, r.arrived));
        versions.chain(responses).max().unwrap_or(0)
    }
}

impl YardLog {
    /// The stages `active` for `agent`: those it made `active` last, whatever type it names.
    pub fn active_stages_of<'a>(&'a self, agent: &'a Agent) -> impl Iterator<Item = &'a Block> {
        self.stages
            .iter()
            .map(|file| &file.stage)
            .filter(|stage| stage.is_active() && stage.owner().is_some_and(|o| o.same(agent)))
    }
}

/// A submission's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Open,
    Answered,
    Withdrawn,
}

impl State {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Answered => "answered",
            Self::Withdrawn => "withdrawn",
        }
    }
}

impl Submission {
    /// Its state: whichever came first of its withdrawal and the answer or outcome that
    /// completed it, the withdrawal where both came in one second, and `open` while neither
    /// has. Once it is no longer open it never is again.
    #[must_use]
    pub fn state(&self, log: &YardLog) -> State {
        let recorded = |what| {
            self.changes
                .iter()
                .filter(move |change| change.what == what)
                .map(|change| change.act.at)
                .min()
        };
        let withdrawn = recorded(Happened::Withdrawn);
        let answered = recorded(Happened::Answered)
            .into_iter()
            .chain(self.completed(log));
        match (withdrawn, answered.min()) {
            (Some(withdrawn), Some(answered)) if answered < withdrawn => State::Answered,
            (Some(_), _) => State::Withdrawn,
            (None, Some(_)) => State::Answered,
            (None, None) => State::Open,
        }
    }

    /// When the last of its blocks was answered or decided by the response that counts, once all
    /// of them are: a plan rewritten and sent again waits for an outcome given after the rewrite.
    fn completed(&self, log: &YardLog) -> Option<Time> {
        self.blocks
            .iter()
            .map(|key| {
                let (_, block) = log.block(*key)?;
                block.counted(log).map(|response| response.act.at)
            })
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .max()
    }

    /// The Act of its send, or of the latest take-up.
    #[must_use]
    pub fn sending(&self) -> &Act {
        self.changes
            .iter()
            .filter(|change| matches!(change.what, Happened::Sent | Happened::TakenUp))
            .max_by_key(|change| (change.act.at, change.act.revision))
            .map(|change| &change.act)
            .expect("a submission records its send")
    }

    /// Who it counts as sent by.
    #[must_use]
    pub fn sender(&self) -> &Actor {
        &self.sending().actor
    }

    /// The latest revision at which a change of it reached this copy.
    #[must_use]
    pub fn reached(&self) -> u64 {
        self.changes
            .iter()
            .map(|change| reached(&change.act, change.arrived))
            .max()
            .unwrap_or(0)
    }
}

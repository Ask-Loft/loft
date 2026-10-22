//! Yard Log as records: a root, and for each stage the stage and its blocks. Nothing is
//! overwritten: a block keeps every version, a submission every change of its state, and an
//! answer or outcome stays as it was given.

use crate::act::{Act, Channel};
use crate::body::{Body, Kind, Verdict};
use crate::format::Format;
use crate::keys::{BlockKey, Handle, ResponseKey, StageKey, SubmissionKey, VersionKey, YardLogKey};

/// One branch's Yard Log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YardLog {
    pub root: Root,
    /// Each stage's file, in the order of [`Root::stages`].
    pub stages: Vec<StageFile>,
}

/// What `yardlog.yaml` holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    pub format: Format,
    pub key: YardLogKey,
    pub branch: String,
    /// The branch's name when Yard Log was opened, where the branch has been renamed since.
    pub opened_as: Option<String>,
    pub title: String,
    pub revision: u64,
    /// The Act of the write that opened it.
    pub opened: Act,
    /// The stages in order, each by its key with its id.
    pub stages: Vec<StageEntry>,
    pub submissions: Vec<Submission>,
    /// The keys of Yard Logs of the same branch, opened apart, that were merged into this one.
    pub merged: Vec<YardLogKey>,
    pub closed: Option<Act>,
    /// The Act of each change this copy holds, in the order it came to hold them: two copies
    /// that each changed Yard Log since their base always differ here, so git never takes
    /// either root whole.
    pub acts: Vec<Act>,
    /// The revision this copy had before a merge that is not settled yet.
    pub unsettled_since: Option<u64>,
    /// The Acts of the changes a merge brought in that this copy did not hold, until settled.
    pub arriving: Vec<Act>,
    pub settlings: Vec<Settling>,
}

/// A stage as the root lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageEntry {
    pub key: StageKey,
    pub id: Handle,
}

/// One stage's file: the stage and its blocks in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageFile {
    pub stage: Block,
    pub blocks: Vec<Block>,
}

/// A block of any kind, a stage included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub key: BlockKey,
    pub id: Handle,
    pub kind: Kind,
    /// A decision's number.
    pub number: Option<u32>,
    /// Every version, in the order written.
    pub versions: Vec<Version>,
    /// The person's answers to a question, or outcomes of an approval or a plan.
    pub responses: Vec<Response>,
}

/// One version of a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub key: VersionKey,
    pub act: Act,
    pub arrived: Option<Arrival>,
    pub body: Body,
}

/// When a record that a merge brought into this copy arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// Brought by a merge that is not settled yet.
    Pending,
    /// At the revision of the settling.
    At(u64),
}

/// An answer to a question, or the outcome of an approval or a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub key: ResponseKey,
    pub act: Act,
    pub channel: Channel,
    pub arrived: Option<Arrival>,
    pub given: Given,
}

/// What the person gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Given {
    Answer {
        /// The labels chosen, in the question's order.
        chosen: Vec<String>,
        /// The person's own words, in place of or beside the labels.
        words: Option<String>,
        notes: Option<String>,
    },
    Outcome {
        verdict: Verdict,
        reason: Option<String>,
    },
}

/// What an agent put to the person at once, and waits on once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub key: SubmissionKey,
    /// Its `question`, `approval` and `plan` blocks, in the batch's order.
    pub blocks: Vec<BlockKey>,
    /// Every change of its state, the send first.
    pub changes: Vec<Change>,
}

/// One change of a submission's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub what: Happened,
    pub act: Act,
    pub arrived: Option<Arrival>,
}

/// What happened to a submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Happened {
    Sent,
    /// Another session's main agent took it up, its sender having stopped.
    TakenUp,
    Answered,
    Withdrawn,
}

impl Happened {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::TakenUp => "taken-up",
            Self::Answered => "answered",
            Self::Withdrawn => "withdrawn",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Sent, Self::TakenUp, Self::Answered, Self::Withdrawn]
            .into_iter()
            .find(|what| what.as_str() == text)
    }
}

/// What settling Yard Log after a merge changed, as one Act.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settling {
    pub act: Act,
    /// Each block given another id, with the old id and the new.
    pub renamed: Vec<(BlockKey, Handle, Handle)>,
    /// Each decision given another number, with the old number and the new.
    pub renumbered: Vec<(BlockKey, u32, u32)>,
}

impl YardLog {
    /// Every block, each stage before its blocks, in order.
    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.stages
            .iter()
            .flat_map(|file| std::iter::once(&file.stage).chain(&file.blocks))
    }

    /// The block whose key is `key`, with the stage that holds it.
    #[must_use]
    pub fn block(&self, key: BlockKey) -> Option<(&StageFile, &Block)> {
        self.stages.iter().find_map(|file| {
            std::iter::once(&file.stage)
                .chain(&file.blocks)
                .find(|block| block.key == key)
                .map(|block| (file, block))
        })
    }

    /// The block whose id is `id`.
    #[must_use]
    pub fn find(&self, id: &Handle) -> Option<&Block> {
        self.blocks().find(|block| &block.id == id)
    }

    pub(crate) fn block_mut(&mut self, key: BlockKey) -> Option<&mut Block> {
        self.stages.iter_mut().find_map(|file| {
            std::iter::once(&mut file.stage)
                .chain(&mut file.blocks)
                .find(|block| block.key == key)
        })
    }

    #[must_use]
    pub fn submission(&self, key: SubmissionKey) -> Option<&Submission> {
        self.root.submissions.iter().find(|s| s.key == key)
    }

    /// Whether Yard Log takes no more writes or answers.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.root.closed.is_some()
    }
}

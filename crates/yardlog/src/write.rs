//! Writing a batch: every block of it, or none. A new id is a new block with a key `loft` mints;
//! an id Yard Log holds gains a version and keeps its key and its earlier versions.

use std::collections::BTreeMap;

use crate::act::{Act, Actor, Agent, Time};
use crate::body::{Body, Decision, Kind, Loft, Recorded};
use crate::datum::Datum;
use crate::format::Format;
use crate::input::{Fault, Input, Request};
use crate::keys::{BlockKey, Handle, SubmissionKey, VersionKey};
use crate::log::{Block, Change, Happened, StageEntry, StageFile, Submission, Version, YardLog};
use crate::query::State;

/// Who writes, when, and how.
#[derive(Debug, Clone)]
pub struct Context {
    pub at: Time,
    pub agent: Agent,
    /// The revision the batch is based on: a block changed after it makes the batch refused.
    pub base: Option<u64>,
    /// Whether the batch is sent: its questions, approvals and plans open a submission.
    pub send: bool,
}

/// What a write wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub revision: u64,
    /// Each block of the batch, by its id, with its key.
    pub blocks: Vec<(Handle, BlockKey)>,
    pub submission: Option<SubmissionKey>,
}

/// Why a batch was refused; a refused batch changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("Yard Log is closed")]
    Closed,
    #[error("Yard Log is written in {0}, which is newer than {current}; a newer `loft` is needed", current = Format::CURRENT)]
    Newer(Format),
    #[error("the batch has {} faults", .0.len())]
    Faults(Vec<Fault>),
    #[error(
        "blocks of the batch were changed after revision {base}; Yard Log is at revision {revision}"
    )]
    Stale {
        base: u64,
        revision: u64,
        blocks: Vec<BlockKey>,
    },
    #[error(
        "block `{block}` is held by open submission {submission}; withdraw it to change the block"
    )]
    Held {
        block: Handle,
        submission: SubmissionKey,
    },
    #[error("the batch is marked to be sent but holds no `question`, `approval` or `plan` block")]
    NothingToSend,
    #[error("the `{view}` view of block `{block}` could not be computed: {error}")]
    Report {
        block: Handle,
        view: &'static str,
        error: String,
    },
}

/// The report of each `loft` block of a batch, by its id, computed before the write: what it
/// records, or why it could not be computed.
pub type Reports = BTreeMap<Handle, Result<(Recorded, Datum), String>>;

impl YardLog {
    /// Yard Log as an agent's first write on `branch` opens it, before the write: revision 0,
    /// with the Act of the write that opens it.
    #[must_use]
    pub fn open(branch: &str, base: &str, at: Time, agent: Agent) -> Self {
        Self {
            root: crate::log::Root {
                format: Format::CURRENT,
                key: crate::keys::YardLogKey::mint(),
                branch: branch.to_owned(),
                base: base.to_owned(),
                opened_as: None,
                title: branch.to_owned(),
                revision: 0,
                opened: Act {
                    at,
                    revision: 1,
                    actor: Actor::Agent(agent),
                },
                stages: Vec::new(),
                submissions: Vec::new(),
                merged: Vec::new(),
                closed: None,
                acts: Vec::new(),
                unsettled_since: None,
                arriving: Vec::new(),
                settlings: Vec::new(),
            },
            stages: Vec::new(),
        }
    }

    /// Writes a batch whole, or refuses it and changes nothing.
    ///
    /// # Errors
    ///
    /// The batch breaks a rule of Yard Log, a `loft` view could not be computed, or Yard Log is
    /// closed or of a newer format.
    pub fn write(
        &mut self,
        inputs: &[Input],
        context: &Context,
        reports: &Reports,
    ) -> Result<Written, Refusal> {
        if !self.root.format.is_known() {
            return Err(Refusal::Newer(self.root.format));
        }
        if self.is_closed() {
            return Err(Refusal::Closed);
        }
        let plan = Plan::check(self, inputs, context)?;
        let mut next = self.clone();
        let written = next.apply(inputs, context, &plan, reports)?;
        let faults = next.active_stages(&context.agent, inputs);
        if !faults.is_empty() {
            return Err(Refusal::Faults(faults));
        }
        *self = next;
        Ok(written)
    }

    fn apply(
        &mut self,
        inputs: &[Input],
        context: &Context,
        plan: &Plan,
        reports: &Reports,
    ) -> Result<Written, Refusal> {
        let act = self.act(context.at, Actor::Agent(context.agent.clone()));
        let revision = act.revision;
        let mut blocks = Vec::new();
        for input in inputs {
            let body = Self::body(input, plan, reports)?;
            let version = Version {
                key: VersionKey::mint(),
                act: act.clone(),
                arrived: None,
                body,
            };
            let key = match plan.existing.get(&input.id) {
                Some(key) => {
                    self.block_mut(*key)
                        .expect("checked")
                        .versions
                        .push(version);
                    *key
                }
                None => self.add(input, version, plan),
            };
            blocks.push((input.id.clone(), key));
        }
        let submission = context.send.then(|| self.send(&blocks, inputs, &act));
        Ok(Written {
            revision,
            blocks,
            submission,
        })
    }

    fn body(input: &Input, plan: &Plan, reports: &Reports) -> Result<Body, Refusal> {
        Ok(match &input.request {
            Request::Stage(stage) => Body::Stage(stage.clone()),
            Request::Note(note) => Body::Note(note.clone()),
            Request::Question(question) => Body::Question(question.clone()),
            Request::Approval(approval) => Body::Approval(approval.clone()),
            Request::Task(task) => Body::Task(task.clone()),
            Request::Plan(plan) => Body::Plan(plan.clone()),
            Request::Decision {
                title,
                consequence,
                question,
            } => Body::Decision(Decision {
                title: title.clone(),
                consequence: consequence.clone(),
                question: question.as_ref().map(|id| plan.keys[id]),
            }),
            Request::Loft(request) => {
                let computed = reports
                    .get(&input.id)
                    .cloned()
                    .unwrap_or_else(|| Err("no report was computed for it".to_owned()));
                let (recorded, report) = computed.map_err(|error| Refusal::Report {
                    block: input.id.clone(),
                    view: request.view.as_str(),
                    error,
                })?;
                Body::Loft(Box::new(Loft {
                    view: request.view,
                    note: request.note.clone(),
                    base: request.base.clone(),
                    decision: request.decision.clone(),
                    reasons: request.reasons.clone(),
                    recorded,
                    report,
                }))
            }
        })
    }

    /// Adds a new block, or a new stage with its file, where the batch places it.
    fn add(&mut self, input: &Input, version: Version, plan: &Plan) -> BlockKey {
        let key = plan.keys[&input.id];
        let kind = input.request.kind();
        let number = (kind == Kind::Decision).then(|| self.next_number());
        let block = Block {
            key,
            id: input.id.clone(),
            kind,
            number,
            versions: vec![version],
            responses: Vec::new(),
        };
        let Some(stage) = &input.stage else {
            let at = input.after.as_ref().map_or(self.stages.len(), |after| {
                self.stages
                    .iter()
                    .position(|file| &file.stage.id == after)
                    .expect("checked")
                    + 1
            });
            self.root.stages.insert(
                at,
                StageEntry {
                    key: key.stage(),
                    id: input.id.clone(),
                },
            );
            self.stages.insert(
                at,
                StageFile {
                    stage: block,
                    blocks: Vec::new(),
                },
            );
            return key;
        };
        let file = self
            .stages
            .iter_mut()
            .find(|file| &file.stage.id == stage)
            .expect("checked");
        let at = input.after.as_ref().map_or(file.blocks.len(), |after| {
            file.blocks
                .iter()
                .position(|b| &b.id == after)
                .expect("checked")
                + 1
        });
        file.blocks.insert(at, block);
        key
    }

    fn next_number(&self) -> u32 {
        self.blocks()
            .filter_map(|block| block.number)
            .max()
            .unwrap_or(0)
            + 1
    }

    fn send(
        &mut self,
        blocks: &[(Handle, BlockKey)],
        inputs: &[Input],
        act: &Act,
    ) -> SubmissionKey {
        let key = SubmissionKey::mint();
        let asked = inputs
            .iter()
            .zip(blocks)
            .filter(|(input, _)| input.request.kind().is_asked())
            .map(|(_, (_, key))| *key)
            .collect();
        self.root.submissions.push(Submission {
            key,
            blocks: asked,
            changes: vec![Change {
                what: Happened::Sent,
                act: act.clone(),
                arrived: None,
            }],
        });
        key
    }

    /// Faults for an agent left with two stages `active`, once a batch is applied: each stage the
    /// batch makes `active` names the agent's other active stages, and how to set them aside in
    /// the same batch.
    fn active_stages(&self, agent: &Agent, inputs: &[Input]) -> Vec<Fault> {
        let active: Vec<&Handle> = self
            .active_stages_of(agent)
            .map(|stage| &stage.id)
            .collect();
        if active.len() < 2 {
            return Vec::new();
        }
        inputs
            .iter()
            .filter(|input| active.contains(&&input.id))
            .map(|input| {
                let others: Vec<String> = active
                    .iter()
                    .filter(|id| ***id != input.id)
                    .map(|id| format!("`{id}`"))
                    .collect();
                let (stage, verb, its) = if others.len() == 1 {
                    ("stage", "is", "its")
                } else {
                    ("stages", "are", "their")
                };
                Fault {
                    block: input.id.to_string(),
                    field: "status".to_owned(),
                    message: format!(
                        "{stage} {} {verb} already active for this agent, which has one stage \
                         active at a time: set {its} status to `done`, `planned` or `dropped` in \
                         the same batch to make `{}` active",
                        others.join(", "),
                        input.id,
                    ),
                }
            })
            .collect()
    }
}

/// What checking a batch against Yard Log found: each id's key, minted for a new block, and
/// which ids Yard Log already holds.
struct Plan {
    keys: BTreeMap<Handle, BlockKey>,
    existing: BTreeMap<Handle, BlockKey>,
}

impl Plan {
    fn check(log: &YardLog, inputs: &[Input], context: &Context) -> Result<Self, Refusal> {
        let mut plan = Self {
            keys: log
                .blocks()
                .map(|block| (block.id.clone(), block.key))
                .collect(),
            existing: BTreeMap::new(),
        };
        let mut faults = Vec::new();
        for input in inputs {
            if inputs.iter().filter(|other| other.id == input.id).count() > 1 {
                faults.push(fault(
                    input,
                    "id",
                    "the batch writes this id more than once",
                ));
                continue;
            }
            match log.find(&input.id) {
                Some(block) => {
                    plan.existing.insert(input.id.clone(), block.key);
                }
                None => {
                    plan.keys.insert(input.id.clone(), BlockKey::mint());
                }
            }
        }
        for input in inputs {
            faults.extend(crate::rules::faults(log, inputs, input));
        }
        if !faults.is_empty() {
            return Err(Refusal::Faults(faults));
        }
        crate::rules::held(log, &plan.existing)?;
        if let Some(base) = context.base {
            crate::rules::stale(log, base, &plan.existing)?;
        }
        if context.send && !inputs.iter().any(|input| input.request.kind().is_asked()) {
            return Err(Refusal::NothingToSend);
        }
        Ok(plan)
    }
}

pub(crate) fn fault(input: &Input, field: &str, message: impl Into<String>) -> Fault {
    Fault {
        block: input.id.to_string(),
        field: field.to_owned(),
        message: message.into(),
    }
}

/// Whether a submission that holds `key` is open.
pub(crate) fn holder(log: &YardLog, key: BlockKey) -> Option<SubmissionKey> {
    log.root
        .submissions
        .iter()
        .find(|s| s.blocks.contains(&key) && s.state(log) == State::Open)
        .map(|s| s.key)
}

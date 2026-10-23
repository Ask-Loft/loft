//! The person's answers and outcomes, and the changes of a submission's state and of Yard Log's
//! that agents make: withdrawing a submission, taking one up, and closing Yard Log.

use crate::act::{Act, Actor, Agent, Channel, Time};
use crate::body::{Body, Verdict};
use crate::keys::{BlockKey, ResponseKey, SubmissionKey};
use crate::log::{Change, Given, Happened, Response, YardLog};
use crate::query::State;

/// One answer or outcome the person gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub block: BlockKey,
    pub given: Given,
}

/// Who answers, when, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answering {
    pub at: Time,
    pub name: String,
    pub email: String,
    pub channel: Channel,
}

/// What answering recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub revision: u64,
    /// The blocks whose answer or outcome was recorded.
    pub recorded: Vec<BlockKey>,
    /// The blocks answered or decided elsewhere first, with the channel of the answer that
    /// counts; their replies were refused.
    pub elsewhere: Vec<(BlockKey, Channel)>,
    /// The submissions these answers completed.
    pub completed: Vec<SubmissionKey>,
}

/// Why a change was refused; a refused change writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    #[error("Yard Log is closed")]
    Closed,
    #[error("Yard Log holds no block {0}")]
    NoBlock(BlockKey),
    #[error("block `{block}`: {message}")]
    Invalid { block: String, message: String },
    #[error("Yard Log holds no submission {0}")]
    NoSubmission(SubmissionKey),
    #[error("submission {submission} is {}, not open", state.as_str())]
    NotOpen {
        submission: SubmissionKey,
        state: State,
    },
    #[error("only a session's main agent takes up a submission")]
    NotMain,
}

impl YardLog {
    /// The Act of the next change: it raises the revision and joins the root's Acts.
    pub(crate) fn act(&mut self, at: Time, actor: Actor) -> Act {
        self.root.revision += 1;
        let act = Act {
            at,
            revision: self.root.revision,
            actor,
        };
        self.root.acts.push(act.clone());
        act
    }

    /// Brings Yard Log in an older format up to the one this `loft` writes, as one Act of
    /// `actor`, and gives the format it was in; nothing where it is already current.
    pub fn upgrade(&mut self, at: Time, actor: Actor) -> Option<crate::Format> {
        let from = self.upgrade_format()?;
        self.act(at, actor);
        Some(from)
    }

    /// Brings Yard Log in an older format up to the one this `loft` writes, and gives the format
    /// it was in, within a change whose own Act records it, as a write's does; nothing where it
    /// is already current.
    pub fn upgrade_format(&mut self) -> Option<crate::Format> {
        let from = self.root.format;
        (from < crate::Format::CURRENT).then(|| {
            self.root.format = crate::Format::CURRENT;
            from
        })
    }

    /// Records the person's answers and outcomes. A block answered or decided first elsewhere
    /// keeps that answer, and the later reply to it is refused and reported.
    ///
    /// # Errors
    ///
    /// Yard Log is closed, or a reply names no block it can answer, or does not fit its block.
    pub fn answer(&mut self, replies: &[Reply], who: &Answering) -> Result<Answered, Refused> {
        if self.is_closed() {
            return Err(Refused::Closed);
        }
        for reply in replies {
            self.check(reply)?;
        }
        let open: Vec<SubmissionKey> = self.open_submissions();
        let mut next = self.clone();
        let person = Actor::Person {
            name: who.name.clone(),
            email: who.email.clone(),
        };
        let mut act = None;
        let mut answered = Answered {
            revision: self.root.revision,
            recorded: Vec::new(),
            elsewhere: Vec::new(),
            completed: Vec::new(),
        };
        for reply in replies {
            let block = next.block_mut(reply.block).expect("checked");
            if let Some(first) = block.counted() {
                answered.elsewhere.push((reply.block, first.channel));
                continue;
            }
            let act = act.get_or_insert_with(|| next.act(who.at, person.clone()));
            let block = next.block_mut(reply.block).expect("checked");
            block.responses.push(Response {
                key: ResponseKey::mint(),
                act: act.clone(),
                channel: who.channel,
                arrived: None,
                given: reply.given.clone(),
            });
            answered.recorded.push(reply.block);
        }
        if let Some(act) = act {
            answered.completed = next.complete(&open, &act);
        }
        answered.revision = next.root.revision;
        *self = next;
        Ok(answered)
    }

    /// The keys of the submissions still open, in the order they were sent.
    #[must_use]
    pub fn open_submissions(&self) -> Vec<SubmissionKey> {
        self.root
            .submissions
            .iter()
            .filter(|s| s.state(self) == State::Open)
            .map(|s| s.key)
            .collect()
    }

    /// Records as answered each of `open` that the answers just given completed.
    fn complete(&mut self, open: &[SubmissionKey], act: &Act) -> Vec<SubmissionKey> {
        let done: Vec<SubmissionKey> = open
            .iter()
            .copied()
            .filter(|key| {
                self.submission(*key)
                    .is_some_and(|s| s.state(self) == State::Answered)
            })
            .collect();
        for submission in &mut self.root.submissions {
            if done.contains(&submission.key) {
                submission.changes.push(Change {
                    what: Happened::Answered,
                    act: act.clone(),
                    arrived: None,
                });
            }
        }
        done
    }

    fn check(&self, reply: &Reply) -> Result<(), Refused> {
        let (_, block) = self
            .block(reply.block)
            .ok_or(Refused::NoBlock(reply.block))?;
        let invalid = |message: &str| Refused::Invalid {
            block: block.id.to_string(),
            message: message.to_owned(),
        };
        match (&block.current().body, &reply.given) {
            (Body::Question(question), Given::Answer { chosen, words, .. }) => {
                if chosen
                    .iter()
                    .any(|label| !question.options.iter().any(|o| &o.label == label))
                {
                    return Err(invalid(
                        "a chosen label is not one of the question's options",
                    ));
                }
                if chosen.len() > 1 && !question.multiple {
                    return Err(invalid("the question takes one option"));
                }
                if words.is_some() && !question.own_words {
                    return Err(invalid(
                        "the question takes no answer in the person's own words",
                    ));
                }
                if chosen.is_empty() && words.is_none() {
                    return Err(invalid(
                        "an answer chooses an option or gives the person's own words",
                    ));
                }
                Ok(())
            }
            (Body::Approval(_) | Body::Plan(_), Given::Outcome { verdict, reason }) => {
                if *verdict == Verdict::Rejected && reason.is_none() {
                    return Err(invalid("a rejection gives the person's reason"));
                }
                Ok(())
            }
            (body, _) if !body.kind().is_asked() => Err(invalid(
                "only a question, an approval or a plan is answered or decided",
            )),
            (Body::Question(_), _) => Err(invalid("a question takes an answer, not an outcome")),
            _ => Err(invalid(
                "an approval or a plan takes an outcome, not an answer",
            )),
        }
    }

    /// Withdraws an open submission, leaving its blocks unanswered.
    ///
    /// # Errors
    ///
    /// Yard Log holds no such submission, or it is not open.
    pub fn withdraw(&mut self, key: SubmissionKey, agent: Agent, at: Time) -> Result<u64, Refused> {
        if self.is_closed() {
            return Err(Refused::Closed);
        }
        self.must_be_open(key)?;
        let act = self.act(at, Actor::Agent(agent));
        self.push(key, Happened::Withdrawn, act);
        Ok(self.root.revision)
    }

    /// Lets the main agent of another session take up an open submission, which from then on
    /// counts as sent by it. Whether its sender has stopped is the caller's to know.
    ///
    /// # Errors
    ///
    /// The agent is a subagent, or the submission is not open.
    pub fn take_up(&mut self, key: SubmissionKey, agent: Agent, at: Time) -> Result<u64, Refused> {
        if self.is_closed() {
            return Err(Refused::Closed);
        }
        if !agent.is_main() {
            return Err(Refused::NotMain);
        }
        self.must_be_open(key)?;
        let act = self.act(at, Actor::Agent(agent));
        self.push(key, Happened::TakenUp, act);
        Ok(self.root.revision)
    }

    /// Closes Yard Log, withdrawing every open submission.
    ///
    /// # Errors
    ///
    /// Yard Log is closed already.
    pub fn close(&mut self, agent: Agent, at: Time) -> Result<u64, Refused> {
        if self.is_closed() {
            return Err(Refused::Closed);
        }
        let open = self.open_submissions();
        let act = self.act(at, Actor::Agent(agent));
        for key in open {
            self.push(key, Happened::Withdrawn, act.clone());
        }
        self.root.closed = Some(act);
        Ok(self.root.revision)
    }

    fn must_be_open(&self, key: SubmissionKey) -> Result<(), Refused> {
        let submission = self.submission(key).ok_or(Refused::NoSubmission(key))?;
        match submission.state(self) {
            State::Open => Ok(()),
            state => Err(Refused::NotOpen {
                submission: key,
                state,
            }),
        }
    }

    fn push(&mut self, key: SubmissionKey, what: Happened, act: Act) {
        let submission = self
            .root
            .submissions
            .iter_mut()
            .find(|s| s.key == key)
            .expect("checked");
        submission.changes.push(Change {
            what,
            act,
            arrived: None,
        });
    }
}

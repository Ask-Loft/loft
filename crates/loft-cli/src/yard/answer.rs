//! `loft yard answer` (the Record an answer given in the terminal Feature): the person's answers
//! and decisions given in the agent host's own dialog, recorded with the person's Act and the
//! channel `terminal`, all at one revision. A block answered or decided first elsewhere keeps
//! that answer, and the later one is refused and named; a block in no open submission refuses
//! the whole run.

use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};
use yardlog::query::State;
use yardlog::respond::{Answered, Answering, Reply};
use yardlog::{Actor, BlockKey, Channel, Handle, Key, YardLog};

use super::answer_input::{self, Named};
use super::args::AnswerArgs;
use super::branch::{self, Branch, Unapplied};
use super::caller::Caller;
use super::change::refusal;
use super::failure::{self, Failure};
use super::working_tree::{Follows, sent_to, target};
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-answer/1";
pub const OUTCOME: &str = "answered";
/// Some answers were recorded, and others refused as answered elsewhere first.
pub const PARTLY: &str = "answered elsewhere";

pub fn run(root: &Path, args: AnswerArgs) -> Finished {
    let as_json = args.output.json();
    match attempt(root, args) {
        Ok((fields, errors)) => {
            let outcome = if errors.is_empty() { OUTCOME } else { PARTLY };
            let code = if errors.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            let finished = Finished::with(code, outcome, &errors);
            if as_json {
                json::print(&json::result(SCHEMA, outcome, fields, errors));
            } else {
                outln!("Recorded the answers at revision {}", fields["revision"]);
            }
            finished
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}

/// The person, as git names them for the repository: their name and email.
pub fn person(branch: &Branch) -> Result<(String, String), Failure> {
    let person = branch.git.person(&branch.place().tree)?;
    match (person.name, person.email) {
        (Some(name), Some(email)) => Ok((name, email)),
        _ => Err(Failure::one(
            "person",
            "git gives no `user.name` and `user.email` for the repository, which name the person",
        )),
    }
}

fn attempt(dir: &Path, args: AnswerArgs) -> Result<(Value, Vec<Value>), Failure> {
    let caller = Caller::new(&args.writer.clone().into());
    let named = answer_input::read(&super::input::json(&args.input)?)?;
    let dir = match &args.writer.tree {
        Some(tree) => target(dir, Some(tree), caller.agent.as_ref(), Follows::Directory)?,
        None => sent_to(dir, caller.agent(), |log| replies(log, &named).is_ok())?,
    };
    let branch = Branch::locate(&dir, args.beyond)?;
    record(&branch, &named, Channel::Terminal)
}

/// Records the person's answers and decisions `named` in the branch's Yard Log, given on
/// `channel`: the result's fields, and an error for each block answered or decided elsewhere
/// first.
pub fn record(
    branch: &Branch,
    named: &[Named],
    channel: Channel,
) -> Result<(Value, Vec<Value>), Failure> {
    let (name, email) = person(branch)?;
    let at = super::now()?;
    let settler = Actor::Person {
        name: name.clone(),
        email: email.clone(),
    };
    let held = branch.open(at, || Ok(settler))?;
    branch.refuse_if_merged(&held)?;
    let who = Answering {
        at,
        name,
        email,
        channel,
    };
    let applied = branch.update(|current| {
        let mut log = current.ok_or(Unapplied::Refused(branch::absent()))?;
        let replies = replies(&log, named).map_err(Unapplied::Refused)?;
        let answered = log
            .answer(&replies, &who)
            .map_err(|refused| Unapplied::Refused(refusal(&refused)))?;
        let fields = answered_fields(&log, &answered);
        Ok((log, fields))
    })?;
    match applied {
        Ok((mut fields, errors)) => {
            let mut located = branch.fields(fields["revision"].as_u64().unwrap_or_default());
            if let (Some(all), Some(own)) = (located.as_object_mut(), fields.as_object_mut()) {
                all.append(own);
            }
            Ok((located, errors))
        }
        Err(error) => Err(branch.refused(&branch.store.read()?, vec![error])),
    }
}

/// The block a host names by its id or its key, where Yard Log holds it.
fn block_named(log: &YardLog, name: &str) -> Option<BlockKey> {
    if let Ok(key) = Key::parse(name) {
        let key = BlockKey::new(key);
        return log.block(key).map(|_| key);
    }
    let id = Handle::parse(name).ok()?;
    log.find(&id).map(|block| block.key)
}

/// The replies, each to a block of a submission sent and not withdrawn: a block of one already
/// answered goes on, to be refused as answered elsewhere.
fn replies(log: &YardLog, named: &[Named]) -> Result<Vec<Reply>, Value> {
    let asked: Vec<BlockKey> = log
        .root
        .submissions
        .iter()
        .filter(|submission| submission.state(log) != State::Withdrawn)
        .flat_map(|submission| submission.blocks.iter().copied())
        .collect();
    named
        .iter()
        .map(|named| {
            let block = block_named(log, &named.block)
                .filter(|key| asked.contains(key))
                .ok_or_else(|| {
                    let message = format!("block `{}` is in no open submission", named.block);
                    failure::error("not-asked", &message)
                })?;
            Ok(Reply {
                block,
                given: named.given.clone(),
            })
        })
        .collect()
}

/// What the run recorded, and an error for each block answered or decided elsewhere first.
fn answered_fields(log: &YardLog, answered: &Answered) -> (Value, Vec<Value>) {
    let id = |key: BlockKey| {
        log.block(key)
            .map_or_else(String::new, |(_, block)| block.id.to_string())
    };
    let recorded: Vec<Value> = answered
        .recorded
        .iter()
        .map(|key| json!({ "key": key.to_string(), "id": id(*key) }))
        .collect();
    let elsewhere: Vec<Value> = answered
        .elsewhere
        .iter()
        .map(|(key, channel)| {
            json!({ "key": key.to_string(), "id": id(*key), "channel": channel.as_str() })
        })
        .collect();
    let errors = answered
        .elsewhere
        .iter()
        .map(|(key, channel)| {
            let message = format!(
                "block `{}` was answered first on the {}; its answer is kept",
                id(*key),
                channel.as_str()
            );
            failure::error("answered-elsewhere", &message)
        })
        .collect();
    let completed: Vec<String> = answered.completed.iter().map(ToString::to_string).collect();
    let fields = json!({
        "revision": answered.revision,
        "recorded": recorded,
        "elsewhere": elsewhere,
        "completed": completed,
    });
    (fields, errors)
}

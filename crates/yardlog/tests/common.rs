//! What the tests share: a clock, agents, and Yard Log written to through batches of JSON.

use serde_json::{Value, json};
use yardlog::YardLog;
use yardlog::act::{Agent, Channel, Time};
use yardlog::body::Recorded;
use yardlog::datum::Datum;
use yardlog::input::{Input, read_batch};
use yardlog::respond::{Answered, Answering, Reply};
use yardlog::write::{Context, Refusal, Written};

/// 2026-10-22T12:00:00Z plus `seconds`.
pub fn at(seconds: u64) -> Time {
    Time::from_unix_seconds(1_792_670_400 + seconds).unwrap()
}

pub fn main_agent() -> Agent {
    Agent::main("session-a")
}

pub fn other_session() -> Agent {
    Agent::main("session-b")
}

pub fn subagent() -> Agent {
    Agent::sub("session-a", "agent-1", "Explore")
}

pub fn inputs(batch: &Value) -> Vec<Input> {
    read_batch(batch).unwrap_or_else(|faults| panic!("{faults:?}"))
}

/// Opened Yard Log of branch `feat/x`, at revision 0.
pub fn opened() -> YardLog {
    YardLog::open("feat/x", "develop", at(0), main_agent())
}

pub fn context(agent: &Agent, seconds: u64) -> Context {
    Context {
        at: at(seconds),
        agent: agent.clone(),
        base: None,
        send: false,
    }
}

/// A reporter that records an empty report, for batches with a `loft` block.
pub fn recorded() -> (Recorded, Datum) {
    (
        Recorded {
            commit: Some("0123abc".to_owned()),
            branch: Some("feat/x".to_owned()),
            dirty: false,
            loft: "2.2.1".to_owned(),
            at: at(0),
        },
        Datum::Map(vec![("nodes".to_owned(), Datum::List(Vec::new()))]),
    )
}

pub fn try_write(log: &mut YardLog, batch: &Value, context: &Context) -> Result<Written, Refusal> {
    let inputs = inputs(batch);
    let reports = inputs
        .iter()
        .map(|input| (input.id.clone(), Ok(recorded())))
        .collect();
    log.write(&inputs, context, &reports)
}

pub fn write(log: &mut YardLog, batch: &Value, context: &Context) -> Written {
    try_write(log, batch, context).unwrap_or_else(|refusal| panic!("{refusal}: {refusal:?}"))
}

/// Yard Log with a stage `design` and a note in it, written by the main agent.
pub fn planned() -> YardLog {
    let mut log = opened();
    write(
        &mut log,
        &json!([
            {"id": "design", "kind": "stage", "title": "Design"},
            {"id": "intro", "kind": "note", "stage": "design", "markdown": "Hello."}
        ]),
        &context(&main_agent(), 1),
    );
    log
}

pub fn question(id: &str) -> Value {
    json!({
        "id": id, "kind": "question", "stage": "design", "prompt": "Which?", "header": "Pick",
        "options": [{"label": "A"}, {"label": "B"}]
    })
}

pub fn person(channel: Channel, seconds: u64) -> Answering {
    Answering {
        at: at(seconds),
        name: "Ada".to_owned(),
        email: "ada@example.com".to_owned(),
        channel,
    }
}

pub fn answer(log: &mut YardLog, id: &str, label: &str, who: &Answering) -> Answered {
    let key = log.find(&yardlog::Handle::parse(id).unwrap()).unwrap().key;
    log.answer(
        &[Reply {
            block: key,
            given: yardlog::log::Given::Answer {
                chosen: vec![label.to_owned()],
                words: None,
                notes: None,
            },
        }],
        who,
    )
    .unwrap()
}

pub fn handle(id: &str) -> yardlog::Handle {
    yardlog::Handle::parse(id).unwrap()
}

//! Yard Log's records as the commands' results give them: each Act with its time, revision and
//! actor; each block with its key, id, kind and current version, or every version; each answer
//! or outcome with who gave it, when and on which channel. A field a record does not have is
//! left out rather than given as null.

use serde_json::{Map, Value, json};
use yardlog::body::{Body, Choice};
use yardlog::log::{Arrival, Block, Given, Response, Version};
use yardlog::{Act, Actor, BlockKey, YardLog};

/// An actor: an agent by its host and session and, for a subagent, its id and type; or the
/// person by name and email.
pub fn actor(actor: &Actor) -> Value {
    match actor {
        Actor::Agent(agent) => {
            let mut fields = json!({ "host": agent.host, "session": agent.session });
            if let Some(sub) = &agent.subagent {
                fields["agent"] = json!(sub.id);
                fields["agentType"] = json!(sub.kind);
            }
            fields
        }
        Actor::Person { name, email } => json!({ "name": name, "email": email }),
    }
}

pub fn act(act: &Act) -> Value {
    json!({
        "at": act.at.to_string(),
        "revision": act.revision,
        "by": actor(&act.actor),
    })
}

/// Inserts `value` under `name` where there is one.
pub(super) fn put(fields: &mut Map<String, Value>, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        fields.insert(name.to_owned(), value);
    }
}

pub(super) fn text(value: Option<&String>) -> Option<Value> {
    value.map(|text| json!(text))
}

fn arrived(fields: &mut Map<String, Value>, arrival: Option<Arrival>) {
    if let Some(Arrival::At(revision)) = arrival {
        fields.insert("arrived".to_owned(), json!(revision));
    }
}

/// One version: its key, number, Act and its kind's fields.
pub fn version(block: &Block, version: &Version) -> Value {
    let mut fields = Map::new();
    fields.insert("key".to_owned(), json!(version.key.to_string()));
    fields.insert("number".to_owned(), json!(block.number_of(version)));
    fields.insert("act".to_owned(), act(&version.act));
    arrived(&mut fields, version.arrived);
    fields.insert("body".to_owned(), body(&version.body));
    Value::Object(fields)
}

/// A version's fields for its kind.
pub fn body(body: &Body) -> Value {
    let mut fields = Map::new();
    match body {
        Body::Stage(stage) => {
            fields.insert("title".to_owned(), json!(stage.title));
            put(&mut fields, "goal", text(stage.goal.as_ref()));
            fields.insert("status".to_owned(), json!(stage.status.as_str()));
        }
        Body::Note(note) => {
            put(&mut fields, "title", text(note.title.as_ref()));
            fields.insert("markdown".to_owned(), json!(note.markdown));
            put(
                &mut fields,
                "tone",
                note.tone.map(|tone| json!(tone.as_str())),
            );
            fields.insert("folded".to_owned(), json!(note.folded));
        }
        Body::Question(question) => {
            fields.insert("prompt".to_owned(), json!(question.prompt));
            fields.insert("header".to_owned(), json!(question.header));
            put(&mut fields, "context", text(question.context.as_ref()));
            let options: Vec<Value> = question.options.iter().map(choice).collect();
            fields.insert("options".to_owned(), Value::Array(options));
            fields.insert("multiple".to_owned(), json!(question.multiple));
            fields.insert("ownWords".to_owned(), json!(question.own_words));
        }
        Body::Approval(approval) => {
            fields.insert("title".to_owned(), json!(approval.title));
            fields.insert("markdown".to_owned(), json!(approval.markdown));
        }
        Body::Decision(decision) => {
            fields.insert("title".to_owned(), json!(decision.title));
            fields.insert("consequence".to_owned(), json!(decision.consequence));
            put(
                &mut fields,
                "question",
                decision.question.map(|key| json!(key.to_string())),
            );
        }
        Body::Task(task) => {
            put(
                &mut fields,
                "assignee",
                task.assignee
                    .as_ref()
                    .map(|agent| actor(&Actor::Agent(agent.clone()))),
            );
            fields.insert("title".to_owned(), json!(task.title));
            put(&mut fields, "detail", text(task.detail.as_ref()));
            fields.insert("state".to_owned(), json!(task.state.as_str()));
            fields.insert("type".to_owned(), json!(task.kind.as_str()));
        }
        Body::Plan(plan) => {
            fields.insert("title".to_owned(), json!(plan.title));
            fields.insert("markdown".to_owned(), json!(plan.markdown));
            put(&mut fields, "path", text(plan.path.as_ref()));
            fields.insert("planMode".to_owned(), json!(plan.plan_mode));
        }
        Body::Loft(loft) => return super::render_loft::loft(loft),
    }
    Value::Object(fields)
}

fn choice(choice: &Choice) -> Value {
    let mut fields = Map::new();
    fields.insert("label".to_owned(), json!(choice.label));
    put(&mut fields, "detail", text(choice.detail.as_ref()));
    fields.insert("recommended".to_owned(), json!(choice.recommended));
    Value::Object(fields)
}

/// An answer or an outcome, with who gave it, when, and on which channel.
pub fn response(response: &Response) -> Value {
    let mut fields = Map::new();
    fields.insert("key".to_owned(), json!(response.key.to_string()));
    fields.insert("act".to_owned(), act(&response.act));
    fields.insert("channel".to_owned(), json!(response.channel.as_str()));
    arrived(&mut fields, response.arrived);
    match &response.given {
        Given::Answer {
            chosen,
            words,
            notes,
        } => {
            fields.insert("labels".to_owned(), json!(chosen));
            put(&mut fields, "words", text(words.as_ref()));
            put(&mut fields, "notes", text(notes.as_ref()));
        }
        Given::Outcome { verdict, reason } => {
            fields.insert("verdict".to_owned(), json!(verdict.as_str()));
            put(&mut fields, "reason", text(reason.as_ref()));
        }
    }
    Value::Object(fields)
}

/// The blocks of `log` that `keys` name, each as [`block`] gives it with its current version,
/// in the order of `keys`.
pub fn blocks(log: &YardLog, keys: &[BlockKey]) -> Value {
    keys.iter()
        .filter_map(|key| log.block(*key))
        .map(|(file, found)| {
            let stage = (file.stage.key != found.key).then_some(&file.stage);
            block(log, found, stage, false)
        })
        .collect()
}

/// A block: its key, id, kind, stage, decision number, current version or every version, the
/// answer or outcome that counts, and a stage's or task's times and a stage's owner.
pub fn block(log: &YardLog, block: &Block, stage: Option<&Block>, every_version: bool) -> Value {
    let mut fields = Map::new();
    fields.insert("key".to_owned(), json!(block.key.to_string()));
    fields.insert("id".to_owned(), json!(block.id.as_str()));
    fields.insert("kind".to_owned(), json!(block.kind.as_str()));
    put(
        &mut fields,
        "stage",
        stage.map(|stage| json!(stage.key.to_string())),
    );
    put(
        &mut fields,
        "number",
        block.number.map(|number| json!(number)),
    );
    fields.insert("version".to_owned(), version(block, block.current()));
    fields.insert("versionCount".to_owned(), json!(block.versions.len()));
    if every_version {
        let versions = block.ordered().into_iter().map(|v| version(block, v));
        fields.insert("versions".to_owned(), Value::Array(versions.collect()));
    }
    let counted = block.counted(log).map(response);
    let name = if block.kind == yardlog::body::Kind::Question {
        "answer"
    } else {
        "outcome"
    };
    put(&mut fields, name, counted);
    if every_version {
        let responses = block.ordered_responses().into_iter().map(response);
        fields.insert("responses".to_owned(), Value::Array(responses.collect()));
    }
    times(&mut fields, block);
    if block.kind == yardlog::body::Kind::Task {
        let executions: Vec<_> = block
            .executions()
            .into_iter()
            .map(|span| {
                let mut value = json!({"started":span.started.to_string()});
                if let Some(ended) = span.ended {
                    value["ended"] = json!(ended.to_string());
                }
                if let Some(agent) = span.assignee {
                    value["assignee"] = actor(&Actor::Agent(agent));
                }
                value
            })
            .collect();
        fields.insert("executions".to_owned(), json!(executions));
    }
    fields.insert("reached".to_owned(), json!(block.reached()));
    Value::Object(fields)
}

/// A stage's or task's times, and a stage's owner.
fn times(fields: &mut Map<String, Value>, block: &Block) {
    let times = block.times();
    let mut at = Map::new();
    put(
        &mut at,
        "listed",
        times.listed.map(|t| json!(t.to_string())),
    );
    put(
        &mut at,
        "started",
        times.started.map(|t| json!(t.to_string())),
    );
    put(&mut at, "ended", times.ended.map(|t| json!(t.to_string())));
    if !at.is_empty() {
        fields.insert("times".to_owned(), Value::Object(at));
    }
    if block.kind == yardlog::body::Kind::Stage {
        put(
            fields,
            "owner",
            block
                .owner()
                .map(|agent| actor(&Actor::Agent(agent.clone()))),
        );
    }
}

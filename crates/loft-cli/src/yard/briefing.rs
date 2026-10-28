//! The briefing `loft yard read --context` adds: where the branch's Yard Log stands for the
//! calling agent, as fields and as text of at most [`LIMIT`] characters that names no host's
//! tools, so that every adapter tells its agents the same things in the same words and adds only
//! what is particular to its host.

use serde_json::{Value, json};
use yardlog::body::{Body, TaskState};
use yardlog::log::{Block, Given};
use yardlog::query::{State, reached};
use yardlog::{Agent, YardLog};

use super::caller::Continuations;

/// The most characters the briefing's text holds.
pub const LIMIT: usize = 2000;

/// Where the briefing is given: the branch, its page's address, whether git ignores `.yard/`,
/// whether Yard Log is closed and the revision the agent last read.
pub struct Where<'a> {
    pub branch: Option<&'a str>,
    pub page: Option<&'a str>,
    pub ignored: bool,
    pub closed: bool,
    pub since: u64,
}

/// The briefing of `agent` on Yard Log `log`, none where the branch has none.
pub fn briefing(
    log: Option<&YardLog>,
    agent: &Agent,
    continuations: &Continuations,
    at: &Where,
) -> Value {
    let Some(log) = log else {
        return json!({
            "page": at.page,
            "revision": 0,
            "since": at.since,
            "noLog": true,
            "closed": false,
            "ignored": at.ignored,
            "stage": null,
            "assigned": [],
            "open": [],
            "responded": [],
            "text": absent(at),
        });
    };
    let stage = log.working_stage(agent, None).and_then(|key| {
        log.stages
            .iter()
            .find(|file| file.stage.key.stage() == key)
            .map(|file| json!({ "id": file.stage.id.as_str(), "title": title(&file.stage) }))
    });
    let mut fields = json!({
        "page": at.page,
        "revision": log.root.revision,
        "since": at.since,
        "noLog": false,
        "closed": at.closed,
        "ignored": at.ignored,
        "stage": stage,
        "assigned": assigned(log, agent),
        "open": open(log, agent, continuations),
        "responded": responded(log, at.since),
    });
    fields["text"] = json!(text(&fields, at));
    fields
}

/// The unfinished tasks assigned to the agent, each with the stage it is recorded in.
fn assigned(log: &YardLog, agent: &Agent) -> Value {
    log.assigned_to(agent)
        .filter(|(_, _, task)| matches!(task.state, TaskState::Open | TaskState::Active))
        .map(|(file, block, task)| {
            json!({
                "id": block.id.as_str(),
                "stage": file.stage.id.as_str(),
                "title": task.title,
                "state": task.state.as_str(),
            })
        })
        .collect()
}

/// Every open submission: its key, how many blocks it holds and whether the agent sent it.
fn open(log: &YardLog, agent: &Agent, continuations: &Continuations) -> Value {
    let caller = continuations.identity(agent);
    log.root
        .submissions
        .iter()
        .filter(|submission| submission.state(log) == State::Open)
        .map(|submission| {
            let mine = continuations.identity_of(submission.sender()) == Some(caller.clone());
            json!({
                "key": submission.key.to_string(),
                "blocks": submission.blocks.len(),
                "sentBy": if mine { "self" } else { "other" },
            })
        })
        .collect()
}

/// The blocks answered or decided after revision `since`, in the order they were.
fn responded(log: &YardLog, since: u64) -> Value {
    let mut found: Vec<(u64, Value)> = log
        .blocks()
        .filter_map(|block| {
            let response = block.counted(log)?;
            let revision = reached(&response.act, response.arrived);
            let given = match &response.given {
                Given::Answer { .. } => "answered",
                Given::Outcome { verdict, .. } => verdict.as_str(),
            };
            (revision > since).then(|| {
                let fields = json!({
                    "id": block.id.as_str(),
                    "kind": block.kind.as_str(),
                    "response": given,
                    "title": title(block),
                });
                (revision, fields)
            })
        })
        .collect();
    found.sort_by_key(|(revision, _)| *revision);
    found.into_iter().map(|(_, fields)| fields).collect()
}

/// A block's title, or a question's prompt, or its id.
fn title(block: &Block) -> String {
    match &block.current().body {
        Body::Stage(stage) => stage.title.clone(),
        Body::Task(task) => task.title.clone(),
        Body::Question(question) => question.prompt.clone(),
        Body::Approval(approval) => approval.title.clone(),
        Body::Plan(plan) => plan.title.clone(),
        Body::Decision(decision) => decision.title.clone(),
        Body::Note(note) => note.title.clone().unwrap_or_else(|| block.id.to_string()),
        Body::Loft(_) => block.id.to_string(),
    }
}

/// `text` cut to `limit` characters, its last one an ellipsis where it is cut.
fn clipped(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().take(limit - 1).collect();
    format!("{kept}…")
}

fn page_line(at: &Where) -> String {
    at.page.map_or_else(
        || "No page server serves its page yet.".to_owned(),
        |url| format!("Its page: {}", clipped(url, 400)),
    )
}

/// The text of a branch with no Yard Log: its page's address, and that it has none yet.
fn absent(at: &Where) -> String {
    let none = match at.branch {
        Some(_) => "The branch has no Yard Log yet: the first write opens it.",
        None => "HEAD is detached: Yard Log belongs to a branch, so there is none here.",
    };
    format!("{}\n{none}", page_line(at))
}

/// One part of the text: its title, and its lines, which are cut to fit.
struct Section {
    title: String,
    lines: Vec<String>,
}

fn text(fields: &Value, at: &Where) -> String {
    let branch = clipped(at.branch.unwrap_or("this branch"), 120);
    let mut head = vec![format!("Yard Log of `{branch}`."), page_line(at)];
    head.push(if at.ignored {
        "git ignores `.yard/`, so Yard Log stays out of the commits.".to_owned()
    } else {
        "Yard Log is committed with the work: commit `.yard/` beside it.".to_owned()
    });
    if at.closed {
        head.push(
            "Yard Log is closed: it takes no more writes, so more work goes on a branch of its own."
                .to_owned(),
        );
        return clipped(&head.join("\n"), LIMIT);
    }
    head.push(format!(
        "Yard Log is open, at revision {}.",
        fields["revision"]
    ));
    head.push(match &fields["stage"] {
        Value::Null => "You have no working stage.".to_owned(),
        stage => {
            let named = format!("`{}` — {}", str(&stage["id"]), str(&stage["title"]));
            format!("Your working stage: {}.", clipped(&named, 240))
        }
    });
    let mut text = head.join("\n");
    let sections = sections(fields, at.since);
    for (n, section) in sections.iter().enumerate() {
        let later: usize = sections[n + 1..]
            .iter()
            .map(|each| length(&each.title) + 40)
            .sum();
        let room = LIMIT.saturating_sub(length(&text) + length(&section.title) + 2 + later);
        text.push('\n');
        text.push_str(&section.title);
        if !section.lines.is_empty() {
            text.push('\n');
            text.push_str(&fitted(&section.lines, room).join("\n"));
        }
    }
    clipped(&text, LIMIT)
}

fn sections(fields: &Value, since: u64) -> Vec<Section> {
    let list = |name: &str| fields[name].as_array().cloned().unwrap_or_default();
    let assigned = list("assigned");
    let open = list("open");
    let responded = list("responded");
    vec![
        Section {
            title: if assigned.is_empty() {
                "No unfinished task is assigned to you.".to_owned()
            } else {
                format!("Unfinished tasks assigned to you ({}):", assigned.len())
            },
            lines: assigned
                .iter()
                .map(|task| {
                    let (id, progress) = (str(&task["id"]), str(&task["state"]));
                    format!("- `{id}`, {progress}, in stage `{}`", str(&task["stage"]))
                })
                .collect(),
        },
        Section {
            title: if open.is_empty() {
                "No submission is open.".to_owned()
            } else {
                format!("Open submissions ({}):", open.len())
            },
            lines: open.iter().map(submission_line).collect(),
        },
        Section {
            title: if responded.is_empty() {
                format!(
                    "Nothing was answered or decided since revision {since}, which you last read."
                )
            } else {
                format!(
                    "Answered or decided since revision {since}, which you last read ({}):",
                    responded.len()
                )
            },
            lines: responded
                .iter()
                .map(|block| {
                    let (id, response) = (str(&block["id"]), str(&block["response"]));
                    format!("- `{id}` {response}: {}", str(&block["title"]))
                })
                .collect(),
        },
    ]
}

fn submission_line(submission: &Value) -> String {
    let count = submission["blocks"].as_u64().unwrap_or_default();
    let plural = if count == 1 { "" } else { "s" };
    let who = if submission["sentBy"] == "self" {
        "sent by you"
    } else {
        "sent by another agent"
    };
    format!(
        "- `{}`, {count} block{plural}, {who}",
        str(&submission["key"])
    )
}

fn str(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn length(text: &str) -> usize {
    text.chars().count()
}

/// `lines` cut to fit `room` characters, the last saying how many it leaves out and where to
/// find them.
fn fitted(lines: &[String], room: usize) -> Vec<String> {
    let mut kept = Vec::new();
    let mut used = 0;
    for (n, line) in lines.iter().enumerate() {
        let left = lines.len() - n;
        let tail = format!("  … and {left} more: read Yard Log for them");
        let reserve = if left > 1 { length(&tail) + 1 } else { 0 };
        if used + length(line) + 1 + reserve > room {
            kept.push(tail);
            return kept;
        }
        used += length(line) + 1;
        kept.push(line.clone());
    }
    kept
}

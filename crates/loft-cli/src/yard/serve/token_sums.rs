//! Yard Log's token sums as the page shows them, summed from every session's totals file by the
//! rules of Yard Log's model: in all, by stage and within each stage by type of subagent, under
//! no stage, by session, by agent and model, and by submission. Each totals file is summed as it
//! gives its counts, whatever its host; one of a newer totals format is left out, and makes the
//! sums partial.

use std::cmp::Reverse;

use serde_json::{Value, json};
use yardlog::totals::TotalsFormat;
use yardlog::usage::Counts;

/// The sums of `files`, the totals files as JSON, with `activities` the activity files: a session
/// that kept activity and no totals file makes the sums partial.
pub fn sums(files: &[Value], activities: &[Value]) -> Value {
    let mut sums = Sums::default();
    for file in files {
        if TotalsFormat::of(file).is_some_and(TotalsFormat::is_known) {
            sums.add(file);
        } else {
            sums.all.get_or_insert_default().add(Counts::unreported());
        }
    }
    for activity in activities {
        let counted = sums.sessions.iter().any(|session| {
            Some(session.host.as_str()) == activity["host"].as_str()
                && Some(session.id.as_str()) == activity["session"].as_str()
        });
        if !counted {
            sums.all.get_or_insert_default().add(Counts::unreported());
        }
    }
    sums.json()
}

#[derive(Default)]
struct Sums {
    all: Option<Counts>,
    no_stage: Option<Counts>,
    stages: Vec<Stage>,
    submissions: Vec<(String, Counts)>,
    agents: Vec<Agent>,
    sessions: Vec<Session>,
}

struct Stage {
    key: String,
    tokens: Counts,
    /// Each type of subagent that worked for the stage, with its counts within the stage's.
    agents: Vec<(String, Counts)>,
}

struct Agent {
    host: String,
    who: String,
    model: String,
    tokens: Counts,
}

struct Session {
    host: String,
    id: String,
    first_ended: Option<String>,
    last_ended: Option<String>,
    tokens: Counts,
}

impl Sums {
    fn add(&mut self, file: &Value) {
        let host = file["host"].as_str().unwrap_or_default();
        let tokens = Counts::from_fields(&file["tokens"]);
        self.all.get_or_insert_default().add(tokens);
        // A file that keeps no counts under no stage counted nothing there: a measured zero
        // of each counter it reports.
        let no_stage = match file.get("noStage") {
            Some(value) => Counts::from_fields(value),
            None => Counts::from_fields(&zeros(&tokens.json())),
        };
        self.no_stage.get_or_insert_default().add(no_stage);
        for stage in array(file, "stages") {
            let Some(key) = stage["stage"].as_str() else {
                continue;
            };
            let at = self.stage(key);
            at.tokens.add(Counts::from_fields(stage));
            let subagents = array(stage, "agents").filter(|agent| agent["agent"] != "main");
            for agent in subagents {
                let kind = agent["type"].as_str().unwrap_or("subagent");
                add_to(&mut at.agents, kind, Counts::from_fields(agent));
            }
        }
        for submission in array(file, "submissions") {
            if let Some(key) = submission["submission"].as_str() {
                add_to(&mut self.submissions, key, Counts::from_fields(submission));
            }
        }
        for agent in array(file, "agents") {
            let who = if agent["agent"] == "main" {
                "main agent"
            } else {
                agent["type"].as_str().unwrap_or("subagent")
            };
            let model = agent["model"].as_str().unwrap_or("unknown");
            self.agent(host, who, model)
                .tokens
                .add(Counts::from_fields(agent));
        }
        let text = |name: &str| file[name].as_str().map(ToOwned::to_owned);
        self.sessions.push(Session {
            host: host.to_owned(),
            id: file["session"].as_str().unwrap_or_default().to_owned(),
            first_ended: text("firstEnded"),
            last_ended: text("lastEnded"),
            tokens,
        });
    }

    fn stage(&mut self, key: &str) -> &mut Stage {
        let at = self.stages.iter().position(|stage| stage.key == key);
        let at = at.unwrap_or_else(|| {
            self.stages.push(Stage {
                key: key.to_owned(),
                tokens: Counts::default(),
                agents: Vec::new(),
            });
            self.stages.len() - 1
        });
        &mut self.stages[at]
    }

    fn agent(&mut self, host: &str, who: &str, model: &str) -> &mut Agent {
        let at = self
            .agents
            .iter()
            .position(|agent| agent.host == host && agent.who == who && agent.model == model);
        let at = at.unwrap_or_else(|| {
            self.agents.push(Agent {
                host: host.to_owned(),
                who: who.to_owned(),
                model: model.to_owned(),
                tokens: Counts::default(),
            });
            self.agents.len() - 1
        });
        &mut self.agents[at]
    }

    fn json(mut self) -> Value {
        let named = |list: &[(String, Counts)], name: &str| -> Vec<Value> {
            list.iter()
                .map(|(key, counts)| json!({ name: key, "tokens": counts.json() }))
                .collect()
        };
        for stage in &mut self.stages {
            stage
                .agents
                .sort_by_key(|(_, counts)| Reverse(counts.total()));
        }
        // The Claude Code main agent first, then the most tokens first.
        let main =
            |agent: &Agent| agent.host == yardlog::act::CLAUDE_CODE && agent.who == "main agent";
        self.agents.sort_by(|a, b| {
            main(b)
                .cmp(&main(a))
                .then(b.tokens.total().cmp(&a.tokens.total()))
        });
        self.sessions
            .sort_by(|a, b| a.first_ended.cmp(&b.first_ended));
        let stages: Vec<_> = self
            .stages
            .iter()
            .map(|stage| {
                json!({
                    "stage": stage.key,
                    "tokens": stage.tokens.json(),
                    "agents": named(&stage.agents, "type"),
                })
            })
            .collect();
        let agents: Vec<_> = self
            .agents
            .iter()
            .map(|agent| {
                let who = if agent.host == yardlog::act::CLAUDE_CODE {
                    agent.who.clone()
                } else {
                    format!("{} {}", agent.host, agent.who)
                };
                json!({ "who": who, "model": agent.model, "tokens": agent.tokens.json() })
            })
            .collect();
        let sessions: Vec<_> = self.sessions.iter().map(Session::json).collect();
        let sum = |counts: Option<Counts>| counts.map_or_else(|| json!({}), Counts::json);
        json!({
            "all": sum(self.all),
            "noStage": sum(self.no_stage),
            "stages": stages,
            "submissions": named(&self.submissions, "submission"),
            "agents": agents,
            "sessions": sessions,
            "partial": self.all.is_some_and(Counts::partial),
            "counted": self.all.is_some_and(Counts::measured),
        })
    }
}

impl Session {
    fn json(&self) -> Value {
        let mut session = json!({
            "host": self.host,
            "session": self.id,
            "tokens": self.tokens.json(),
        });
        if let Some(at) = &self.first_ended {
            session["firstEnded"] = json!(at);
        }
        if let Some(at) = &self.last_ended {
            session["lastEnded"] = json!(at);
        }
        session
    }
}

/// A measured zero of each counter `counts` holds.
fn zeros(counts: &Value) -> Value {
    let fields = counts.as_object().into_iter().flatten();
    let zeros = fields
        .filter(|(name, _)| yardlog::usage::COUNTERS.contains(&name.as_str()))
        .map(|(name, _)| (name.clone(), json!(0)));
    Value::Object(zeros.collect())
}

fn array<'a>(value: &'a Value, name: &str) -> impl Iterator<Item = &'a Value> {
    value[name].as_array().into_iter().flatten()
}

fn add_to(list: &mut Vec<(String, Counts)>, key: &str, counts: Counts) {
    if let Some((_, held)) = list.iter_mut().find(|(held, _)| held == key) {
        held.add(counts);
        return;
    }
    let mut sum = Counts::default();
    sum.add(counts);
    list.push((key.to_owned(), sum));
}

#[cfg(test)]
mod tests;

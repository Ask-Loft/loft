//! A session's token totals in one branch's Yard Log, as its model requests add to them: in all,
//! by the stage active for the requesting agent, by agent and model, and by each submission open
//! when the request ended. A compaction's own request counts as the main agent's. The page server
//! keeps them in `tokens/<session id>.yaml` in Yard Log's folder.

use std::collections::BTreeMap;

use loftspec::yaml::{Out, write};
use yardlog::Time;

use super::event::{Event, Tokens, Who};

/// A session's token totals in one branch's Yard Log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Totals {
    session: String,
    host: String,
    first_ended: Option<Time>,
    last_ended: Option<Time>,
    all: Tokens,
    stages: BTreeMap<String, Tokens>,
    no_stage: Option<Tokens>,
    agents: BTreeMap<(Who, String), Tokens>,
    submissions: BTreeMap<String, Tokens>,
}

impl Totals {
    pub fn new(session: &str) -> Self {
        Self {
            session: session.to_owned(),
            ..Self::default()
        }
    }

    /// Whether any request was counted, so that the file is written.
    pub const fn counted(&self) -> bool {
        self.last_ended.is_some()
    }

    /// Counts `event` where it is a model request or a compaction, `stage` the stage active for
    /// its agent and `open` the submissions open when it ended; says whether it counted.
    pub fn apply(&mut self, event: &Event, stage: Option<&str>, open: &[String]) -> bool {
        let who = match event.kind.as_str() {
            "request" => event.who.clone(),
            "compaction" => Who::Main,
            _ => return false,
        };
        let Some(tokens) = event.tokens() else {
            return false;
        };
        if self.host.is_empty() {
            self.host.clone_from(&event.host);
        }
        self.first_ended = Some(self.first_ended.map_or(event.at, |at| at.min(event.at)));
        self.last_ended = Some(self.last_ended.map_or(event.at, |at| at.max(event.at)));
        self.all.add(tokens);
        match stage {
            Some(stage) => self.stages.entry(stage.to_owned()).or_default().add(tokens),
            None => self
                .no_stage
                .get_or_insert_with(Tokens::default)
                .add(tokens),
        }
        self.agents
            .entry((who, event.model()))
            .or_default()
            .add(tokens);
        for submission in open {
            self.submissions
                .entry(submission.clone())
                .or_default()
                .add(tokens);
        }
        true
    }

    /// The totals file's text.
    pub fn text(&self) -> String {
        let mut fields = vec![
            ("session", Out::Token(self.session.clone())),
            ("host", Out::Token(self.host.clone())),
        ];
        if let Some(at) = self.first_ended {
            fields.push(("firstEnded", Out::Token(at.to_string())));
        }
        if let Some(at) = self.last_ended {
            fields.push(("lastEnded", Out::Token(at.to_string())));
        }
        fields.push(("tokens", Out::Mapping(counts(self.all))));
        if !self.stages.is_empty() {
            let stages = self.stages.iter().map(|(stage, tokens)| {
                let mut fields = vec![("stage", Out::Token(stage.clone()))];
                fields.extend(counts(*tokens));
                Out::Mapping(fields)
            });
            fields.push(("stages", Out::Sequence(stages.collect())));
        }
        if let Some(tokens) = self.no_stage {
            fields.push(("noStage", Out::Mapping(counts(tokens))));
        }
        let agents = self.agents.iter().map(|((who, model), tokens)| {
            let mut fields = match who {
                Who::Main => vec![("agent", Out::Token("main".to_owned()))],
                Who::Sub { id, kind } => vec![
                    ("agent", Out::Token(id.clone())),
                    ("type", Out::Token(kind.clone())),
                ],
            };
            fields.push(("model", Out::Token(model.clone())));
            fields.extend(counts(*tokens));
            Out::Mapping(fields)
        });
        fields.push(("agents", Out::Sequence(agents.collect())));
        if !self.submissions.is_empty() {
            let submissions = self.submissions.iter().map(|(submission, tokens)| {
                let mut fields = vec![("submission", Out::Token(submission.clone()))];
                fields.extend(counts(*tokens));
                Out::Mapping(fields)
            });
            fields.push(("submissions", Out::Sequence(submissions.collect())));
        }
        write(&Out::Mapping(fields))
    }
}

fn counts(tokens: Tokens) -> Vec<(&'static str, Out)> {
    vec![
        ("input", Out::Integer(tokens.input)),
        ("cacheRead", Out::Integer(tokens.cache_read)),
        ("cacheWrite", Out::Integer(tokens.cache_write)),
        ("output", Out::Integer(tokens.output)),
    ]
}

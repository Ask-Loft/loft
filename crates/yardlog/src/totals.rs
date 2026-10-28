//! A session's token totals in one branch's Yard Log, as its model requests add to them: in all,
//! by the stage the request counts toward and within it by agent, by agent and model, and by each
//! submission open when the request ended. A compaction's own request counts as the main agent's.
//! A session's totals file in Yard Log's folder holds them, `tokens/<session id>.yaml`, and names
//! its totals format, `yardtokens/<n>`.

use std::collections::BTreeMap;
use std::fmt;

use loft_common::yaml::{Out, write};
use serde_json::Value;

use crate::Time;
use crate::usage::Counts;

/// A version of the totals format, `yardtokens/<n>`, which a totals file names as its `format`. A
/// `loft` reads its own totals format and every older one, writes only its own, and leaves a file
/// of a newer one as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TotalsFormat(u32);

impl TotalsFormat {
    /// The totals format this `loft` writes, and the newest it reads.
    pub const CURRENT: Self = Self(2);

    #[must_use]
    pub const fn new(version: u32) -> Self {
        Self(version)
    }

    /// The totals format the totals file `file`, as JSON, names: `yardtokens/1` where it names
    /// none, as the files written before totals named their format; none where it names one
    /// that is not `yardtokens/<n>`.
    #[must_use]
    pub fn of(file: &Value) -> Option<Self> {
        let Some(named) = file.get("format") else {
            return Some(Self(1));
        };
        let digits = named.as_str()?.strip_prefix("yardtokens/")?;
        let canonical = !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
            && !digits.starts_with('0');
        canonical.then(|| digits.parse().ok().map(Self)).flatten()
    }

    /// Whether this `loft` may write a totals file of this format.
    #[must_use]
    pub fn is_known(self) -> bool {
        self <= Self::CURRENT
    }
}

impl fmt::Display for TotalsFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "yardtokens/{}", self.0)
    }
}

/// The agent a request counted toward.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Who {
    Main,
    Sub { id: String, kind: String },
}

impl Who {
    #[must_use]
    pub const fn is_main(&self) -> bool {
        matches!(self, Self::Main)
    }

    /// Reads what [`Who::fields`] wrote: the main agent, or a subagent by its id and type.
    #[must_use]
    pub fn from_fields(value: &Value) -> Self {
        match (value["agent"].as_str(), value["type"].as_str()) {
            (Some("main") | None, None) => Self::Main,
            (id, kind) => Self::Sub {
                id: id.unwrap_or_default().to_owned(),
                kind: kind.unwrap_or_default().to_owned(),
            },
        }
    }

    /// The fields a session's file names the agent by: `agent`, and a subagent's `type`.
    #[must_use]
    pub fn fields(&self) -> Vec<(&'static str, Out)> {
        match self {
            Self::Main => vec![("agent", Out::Token("main".to_owned()))],
            Self::Sub { id, kind } => vec![
                ("agent", Out::Token(id.clone())),
                ("type", Out::Token(kind.clone())),
            ],
        }
    }
}

/// A model request or a compaction, as the totals count it.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The agent host the session runs in.
    pub host: &'a str,
    /// When it ended.
    pub at: Time,
    /// The agent that made it; a compaction's own request counts as the main agent's whatever
    /// this names.
    pub who: &'a Who,
    /// The model that answered it.
    pub model: &'a str,
    /// Whether it is a compaction rather than a model request.
    pub compaction: bool,
    /// Its tokens, where it reports them.
    pub tokens: Option<Counts>,
}

/// What counted toward one stage: in all, and by agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Stage {
    all: Counts,
    agents: BTreeMap<Who, Counts>,
}

/// A session's token totals in one branch's Yard Log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Totals {
    session: String,
    host: String,
    first_ended: Option<Time>,
    last_ended: Option<Time>,
    all: Counts,
    stages: BTreeMap<String, Stage>,
    no_stage: Option<Counts>,
    agents: BTreeMap<(Who, String), Counts>,
    submissions: BTreeMap<String, Counts>,
}

impl Totals {
    #[must_use]
    pub fn new(session: &str) -> Self {
        Self {
            session: session.to_owned(),
            ..Self::default()
        }
    }

    /// Whether any request was counted, so that the file is written.
    #[must_use]
    pub const fn counted(&self) -> bool {
        self.last_ended.is_some()
    }

    /// Counts `request`, `stage` the stage it counts toward and `open` the submissions open when
    /// it ended; says whether it counted.
    pub fn apply(&mut self, request: &Request<'_>, stage: Option<&str>, open: &[String]) -> bool {
        let who = if request.compaction {
            Who::Main
        } else {
            request.who.clone()
        };
        // A request always costs tokens, so one that reports none leaves the totals partial; a
        // compaction that reports none may have made no request of its own.
        let tokens = match request.tokens {
            Some(tokens) => tokens,
            None if request.compaction => return false,
            None => Counts::unreported(),
        };
        if self.host.is_empty() {
            request.host.clone_into(&mut self.host);
        }
        if tokens.measured() || self.counted() {
            let at = request.at;
            self.first_ended = Some(self.first_ended.map_or(at, |first| first.min(at)));
            self.last_ended = Some(self.last_ended.map_or(at, |last| last.max(at)));
        }
        self.all.add(tokens);
        match stage {
            Some(stage) => {
                let counted = self.stages.entry(stage.to_owned()).or_default();
                counted.all.add(tokens);
                counted.agents.entry(who.clone()).or_default().add(tokens);
            }
            None => self
                .no_stage
                .get_or_insert_with(Counts::default)
                .add(tokens),
        }
        self.agents
            .entry((who, request.model.to_owned()))
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

    /// Makes each of the counts these totals hold what `map` makes of it: in all, by stage and
    /// within each by agent, under no stage, by agent and model, and by submission.
    pub fn map_counts(&mut self, map: impl Fn(Counts) -> Counts) {
        self.all = map(self.all);
        for stage in self.stages.values_mut() {
            stage.all = map(stage.all);
            for counts in stage.agents.values_mut() {
                *counts = map(*counts);
            }
        }
        self.no_stage = self.no_stage.map(&map);
        let scopes = self
            .agents
            .values_mut()
            .chain(self.submissions.values_mut());
        for counts in scopes {
            *counts = map(*counts);
        }
    }

    /// Reads a totals file of a known totals format as JSON, as [`Totals::text`] wrote it, so
    /// that writing it again gives the same text, in the current totals format.
    #[must_use]
    pub fn read(file: &Value) -> Self {
        let time = |name: &str| file[name].as_str().and_then(|at| at.parse().ok());
        let items = |name: &str| file[name].as_array().cloned().unwrap_or_default();
        let stages = items("stages").into_iter().filter_map(|stage| {
            let agents = stage["agents"].as_array().into_iter().flatten();
            let agents = agents.map(|agent| (Who::from_fields(agent), Counts::from_fields(agent)));
            let counted = Stage {
                all: Counts::from_fields(&stage),
                agents: agents.collect(),
            };
            Some((stage["stage"].as_str()?.to_owned(), counted))
        });
        let agents = items("agents").into_iter().map(|agent| {
            let model = agent["model"].as_str().unwrap_or("unknown").to_owned();
            (
                (Who::from_fields(&agent), model),
                Counts::from_fields(&agent),
            )
        });
        let submissions = items("submissions").into_iter().filter_map(|submission| {
            let key = submission["submission"].as_str()?.to_owned();
            Some((key, Counts::from_fields(&submission)))
        });
        Self {
            session: file["session"].as_str().unwrap_or_default().to_owned(),
            host: file["host"].as_str().unwrap_or_default().to_owned(),
            first_ended: time("firstEnded"),
            last_ended: time("lastEnded"),
            all: file
                .get("tokens")
                .map_or_else(Counts::default, Counts::from_fields),
            stages: stages.collect(),
            no_stage: file.get("noStage").map(Counts::from_fields),
            agents: agents.collect(),
            submissions: submissions.collect(),
        }
    }

    /// The totals file's text.
    #[must_use]
    pub fn text(&self) -> String {
        let mut fields = vec![
            ("format", Out::Token(TotalsFormat::CURRENT.to_string())),
            ("session", Out::Token(self.session.clone())),
            ("host", Out::Token(self.host.clone())),
        ];
        if let Some(at) = self.first_ended {
            fields.push(("firstEnded", Out::Token(at.to_string())));
        }
        if let Some(at) = self.last_ended {
            fields.push(("lastEnded", Out::Token(at.to_string())));
        }
        let all = self.all.fields();
        if !all.is_empty() {
            fields.push(("tokens", Out::Mapping(all)));
        }
        if !self.stages.is_empty() {
            let stages = self
                .stages
                .iter()
                .map(|(stage, counted)| stage_out(stage, counted));
            fields.push(("stages", Out::Sequence(stages.collect())));
        }
        if let Some(tokens) = self.no_stage {
            let fields_of = tokens.fields();
            if !fields_of.is_empty() {
                fields.push(("noStage", Out::Mapping(fields_of)));
            }
        }
        let agents = self.agents.iter().map(|((who, model), tokens)| {
            let mut fields = who.fields();
            fields.push(("model", Out::Token(model.clone())));
            fields.extend(tokens.fields());
            Out::Mapping(fields)
        });
        fields.push(("agents", Out::Sequence(agents.collect())));
        if !self.submissions.is_empty() {
            let submissions = self.submissions.iter().map(|(submission, tokens)| {
                let mut fields = vec![("submission", Out::Token(submission.clone()))];
                fields.extend(tokens.fields());
                Out::Mapping(fields)
            });
            fields.push(("submissions", Out::Sequence(submissions.collect())));
        }
        write(&Out::Mapping(fields))
    }
}

/// One stage's entry: its key, what counted toward it in all, and by agent.
fn stage_out(stage: &str, counted: &Stage) -> Out {
    let mut fields = vec![("stage", Out::Token(stage.to_owned()))];
    fields.extend(counted.all.fields());
    let agents = counted.agents.iter().map(|(who, tokens)| {
        let mut fields = who.fields();
        fields.extend(tokens.fields());
        Out::Mapping(fields)
    });
    fields.push(("agents", Out::Sequence(agents.collect())));
    Out::Mapping(fields)
}

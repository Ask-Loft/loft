//! A session's activity in one branch's Yard Log, as its events build it: each of the main
//! agent's turns, each subagent's run, each stretch of the main agent's plan mode and of its spec
//! mode, and each shell command that runs `loft`, with when it started and when it ended. The
//! page server keeps it in `activity/<session id>.yaml` in Yard Log's folder.

use std::collections::BTreeMap;

use loftspec::yaml::{Out, write};
use yardlog::Time;

use super::event::{Event, Who};

/// A stretch of time, still going on where it has not ended.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    started: Time,
    ended: Option<Time>,
}

impl Span {
    const fn from(at: Time) -> Self {
        Self {
            started: at,
            ended: None,
        }
    }

    fn fields(&self) -> Vec<(&'static str, Out)> {
        let mut fields = vec![("started", time(self.started))];
        if let Some(ended) = self.ended {
            fields.push(("ended", time(ended)));
        }
        fields
    }
}

/// A subagent's run, with the stage it worked for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    id: String,
    kind: String,
    span: Span,
    stage: Option<String>,
}

/// A shell command that ran `loft`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoftRun {
    commands: Vec<String>,
    who: Who,
    span: Span,
    failed: Option<bool>,
    stage: Option<String>,
}

/// A session's activity in one branch's Yard Log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
    session: String,
    host: String,
    last_ended: Option<Time>,
    turns: Vec<Span>,
    runs: Vec<Run>,
    plan: Vec<Span>,
    spec: Vec<Span>,
    loft: Vec<LoftRun>,
    /// The `loft` runs still going on, by their tool call's id.
    open_loft: BTreeMap<String, usize>,
    /// Whether an event of the session was kept yet.
    heard: bool,
}

fn time(at: Time) -> Out {
    Out::Token(at.to_string())
}

/// Ends the last span of `spans` at `at`, where it is still going on.
fn end(spans: &mut [Span], at: Time) -> bool {
    match spans.last_mut() {
        Some(span) if span.ended.is_none() => {
            span.ended = Some(at);
            true
        }
        _ => false,
    }
}

/// Starts a span at `at`, unless one is still going on.
fn start(spans: &mut Vec<Span>, at: Time) -> bool {
    if spans.last().is_some_and(|span| span.ended.is_none()) {
        return false;
    }
    spans.push(Span::from(at));
    true
}

impl Activity {
    pub fn new(session: &str) -> Self {
        Self {
            session: session.to_owned(),
            ..Self::default()
        }
    }

    /// Keeps what `event` starts or ends, `stage` the stage it counts toward; says whether
    /// it changed anything.
    pub fn apply(&mut self, event: &Event, stage: Option<&str>) -> bool {
        self.host.clone_from(&event.host);
        let at = event.at;
        let main = event.who.is_main();
        let mut changed = false;
        if !self.heard {
            self.heard = true;
            // A turn under way when Yard Log opened is kept from the first event heard of it.
            if main && event.kind != "turn.start" {
                changed |= start(&mut self.turns, at);
            }
        }
        changed |= match event.kind.as_str() {
            "turn.start" if main => start(&mut self.turns, at),
            "turn.end" if main => {
                let spec = end(&mut self.spec, at);
                end(&mut self.turns, at) | spec
            }
            "subagent.start" => self.run_started(event, stage),
            "subagent.end" => self.run_ended(event),
            "plan.start" if main => {
                let spec = end(&mut self.spec, at);
                start(&mut self.plan, at) | spec
            }
            "plan.end" if main => end(&mut self.plan, at),
            "tool.start" => self.tool_started(event, stage),
            "tool.end" => self.tool_ended(event),
            _ => false,
        };
        if changed {
            self.last_ended = Some(at);
        }
        changed
    }

    fn run_started(&mut self, event: &Event, stage: Option<&str>) -> bool {
        let Who::Sub { id, kind } = &event.who else {
            return false;
        };
        self.runs.push(Run {
            id: id.clone(),
            kind: kind.clone(),
            span: Span::from(event.at),
            stage: stage.map(ToOwned::to_owned),
        });
        true
    }

    fn run_ended(&mut self, event: &Event) -> bool {
        let Who::Sub { id, .. } = &event.who else {
            return false;
        };
        let open = self
            .runs
            .iter_mut()
            .rev()
            .find(|run| &run.id == id && run.span.ended.is_none());
        open.is_some_and(|run| {
            run.span.ended = Some(event.at);
            true
        })
    }

    fn tool_started(&mut self, event: &Event, stage: Option<&str>) -> bool {
        let mut changed = false;
        // Spec mode is the main agent's alone: from its first edit under `spec/` to its next
        // edit of another file.
        if event.who.is_main() {
            changed |= match event.spec() {
                Some(true) => start(&mut self.spec, event.at),
                Some(false) => end(&mut self.spec, event.at),
                None => false,
            };
        }
        let commands = event.loft();
        if let (false, Some(call)) = (commands.is_empty(), event.call()) {
            self.open_loft.insert(call.to_owned(), self.loft.len());
            self.loft.push(LoftRun {
                commands,
                who: event.who.clone(),
                span: Span::from(event.at),
                failed: None,
                stage: stage.map(ToOwned::to_owned),
            });
            changed = true;
        }
        changed
    }

    fn tool_ended(&mut self, event: &Event) -> bool {
        let Some(at) = event.call().and_then(|call| self.open_loft.remove(call)) else {
            return false;
        };
        let run = &mut self.loft[at];
        run.span.ended = Some(event.at);
        run.failed = Some(event.failed());
        true
    }

    /// The activity file's text.
    pub fn text(&self) -> String {
        let mut fields = vec![("session", Out::Token(self.session.clone()))];
        if self.host != yardlog::act::CLAUDE_CODE && !self.host.is_empty() {
            fields.push(("host", Out::Token(self.host.clone())));
        }
        if let Some(at) = self.last_ended {
            fields.push(("lastEnded", time(at)));
        }
        let spans = |spans: &[Span]| {
            Out::Sequence(
                spans
                    .iter()
                    .map(|span| Out::Mapping(span.fields()))
                    .collect(),
            )
        };
        if !self.turns.is_empty() {
            fields.push(("turns", spans(&self.turns)));
        }
        if !self.runs.is_empty() {
            let runs = self.runs.iter().map(subagent_run);
            fields.push(("subagents", Out::Sequence(runs.collect())));
        }
        if !self.plan.is_empty() {
            fields.push(("plan", spans(&self.plan)));
        }
        if !self.spec.is_empty() {
            fields.push(("spec", spans(&self.spec)));
        }
        if !self.loft.is_empty() {
            let runs = self.loft.iter().map(loft_run);
            fields.push(("loft", Out::Sequence(runs.collect())));
        }
        write(&Out::Mapping(fields))
    }
}

fn subagent_run(run: &Run) -> Out {
    let mut fields = vec![
        ("agent", Out::Token(run.id.clone())),
        ("type", Out::Token(run.kind.clone())),
    ];
    fields.extend(run.span.fields());
    if let Some(stage) = &run.stage {
        fields.push(("stage", Out::Token(stage.clone())));
    }
    Out::Mapping(fields)
}

fn loft_run(run: &LoftRun) -> Out {
    let commands = run.commands.iter().cloned().map(Out::Token).collect();
    let mut fields = vec![("commands", Out::Sequence(commands))];
    fields.extend(run.who.fields());
    fields.extend(run.span.fields());
    if let Some(failed) = run.failed {
        fields.push(("failed", Out::Plain(failed.to_string())));
    }
    if let Some(stage) = &run.stage {
        fields.push(("stage", Out::Token(stage.clone())));
    }
    Out::Mapping(fields)
}

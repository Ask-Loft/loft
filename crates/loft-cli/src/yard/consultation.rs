//! `loft yard consultation` (the Report what the agents consulted Feature): the sums of the
//! branch's consultation files, in all, by stage, by session and by agent type, as `yardlog`'s
//! [`Sums`] counts them. The page server gives the page the same report of each Yard Log it
//! serves, through [`report`]. It reads Yard Log as it stands, closed or open, and writes nothing.

use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Map, Value, json};
use yardlog::YardLog;
use yardlog::body::Body;
use yardlog::consult::{Estimate, Scope, SessionSum, Six, Sums};
use yardlog_store::git::YardGit;
use yardlog_store::session::SessionFile;
use yardlog_store::{Held, Place, YardStore};

use super::args::{Beyond, ConsultationArgs, DEFAULT_PORT};
use super::branch::Branch;
use super::failure::Failure;
use super::session_files;
use super::working_tree::{Follows, target};
use crate::finish::Finished;
use crate::json;
use crate::out::errln;

const SCHEMA: &str = "loft-yard-consultation/1";
const OUTCOME: &str = "reported";

pub fn run(root: &Path, args: &ConsultationArgs) -> Finished {
    let as_json = args.output.json();
    match document(root, args) {
        Ok(fields) => {
            if as_json {
                json::print(&json::result(SCHEMA, OUTCOME, fields, Vec::new()));
            } else {
                for line in table(&fields) {
                    errln!("{line}");
                }
            }
            Finished::success(OUTCOME)
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}

/// The report of the Yard Log of the branch the command's tree is on, with where it lies.
fn document(root: &Path, args: &ConsultationArgs) -> Result<Value, Failure> {
    let dir = target(root, args.tree.as_deref(), None, Follows::Directory)?;
    let git = YardGit::default();
    let place = Place::locate(&git, &dir)?;
    let branch = Branch::at(git, place, Beyond { port: DEFAULT_PORT })?;
    let stages = match branch.store.read()? {
        Held::Log(log) => titled(&log),
        Held::Absent | Held::Newer(_) => Vec::new(),
    };
    let mut fields = branch.located();
    for (name, value) in as_object(report(&branch.store, &stages)) {
        fields[name] = value;
    }
    Ok(fields)
}

fn as_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(fields) => fields,
        _ => Map::new(),
    }
}

/// Each stage of `log` by its key, with its title, in Yard Log's order.
pub fn titled(log: &YardLog) -> Vec<(String, String)> {
    log.root
        .stages
        .iter()
        .zip(&log.stages)
        .map(|(entry, file)| {
            let title = file
                .stage
                .versions
                .last()
                .and_then(|version| match &version.body {
                    Body::Stage(stage) => Some(stage.title.clone()),
                    _ => None,
                });
            (entry.key.to_string(), title.unwrap_or_default())
        })
        .collect()
}

/// The sums of the consultation files in the Yard Log of `store`, each stage of `stages` by its
/// key and title in their order, then any stage the files count toward that `stages` does not
/// hold, by its key alone.
pub fn report(store: &YardStore, stages: &[(String, String)]) -> Value {
    let sums = Sums::of(
        &session_files::read(store, SessionFile::Consultation),
        &session_files::read(store, SessionFile::Activity),
    );
    let unmeasured: Vec<Value> = sums
        .unmeasured
        .iter()
        .map(|(session, host)| json!({ "session": session, "host": host }))
        .collect();
    if !sums.measured() {
        return json!({
            "measured": false,
            "partial": sums.partial,
            "stages": [],
            "sessions": [],
            "unmeasured": unmeasured,
            "agents": [],
        });
    }
    let mut fields = json!({
        "measured": true,
        "partial": sums.partial,
        "all": tallies(sums.counts, sums.estimate),
        "stages": stage_list(&sums, stages),
        "sessions": sums.sessions.iter().map(session).collect::<Vec<_>>(),
        "unmeasured": unmeasured,
        "agents": agent_list(&sums),
    });
    if let Some(scope) = &sums.no_stage {
        fields["noStage"] = scoped(scope);
    }
    fields
}

/// The stages the files count toward: those of `stages` in their order, then the rest.
fn stage_list(sums: &Sums, stages: &[(String, String)]) -> Vec<Value> {
    let known = stages.iter().filter_map(|(key, title)| {
        let scope = sums.stages.get(key)?;
        Some(titled_scope(key, Some(title), scope))
    });
    let rest = sums
        .stages
        .iter()
        .filter(|(key, _)| !stages.iter().any(|(known, _)| known == *key))
        .map(|(key, scope)| titled_scope(key, None, scope));
    known.chain(rest).collect()
}

fn titled_scope(key: &str, title: Option<&String>, scope: &Scope) -> Value {
    let mut fields = json!({ "stage": key });
    if let Some(title) = title {
        fields["title"] = json!(title);
    }
    for (name, value) in as_object(scoped(scope)) {
        fields[name] = value;
    }
    fields
}

/// The agent types, the main agent first.
fn agent_list(sums: &Sums) -> Vec<Value> {
    let main = sums.agents.get_key_value("main");
    let others = sums.agents.iter().filter(|(kind, _)| *kind != "main");
    main.into_iter()
        .chain(others)
        .map(|(kind, scope)| {
            let mut fields = tallies(scope.counts, scope.estimate);
            fields["type"] = json!(kind);
            fields
        })
        .collect()
}

fn session(sum: &SessionSum) -> Value {
    let mut fields = tallies(sum.counts, sum.estimate);
    fields["session"] = json!(sum.session);
    fields["host"] = json!(sum.host);
    fields["partial"] = json!(sum.partial);
    fields
}

/// The fields of `values` under `names`.
fn named<const N: usize>(names: [&str; N], values: [u64; N]) -> Value {
    names
        .into_iter()
        .zip(values)
        .map(|(name, value)| (name.to_owned(), json!(value)))
        .collect::<Map<_, _>>()
        .into()
}

fn tallies(counts: Six, estimate: Estimate) -> Value {
    json!({
        "counts": named(Six::NAMES, counts.values()),
        "estimate": named(Estimate::NAMES, estimate.values()),
    })
}

/// A scope's tallies and its order: the times it holds, the edits of code before the first
/// consultation of the specification and, where code was edited, whether the specification was
/// consulted first.
fn scoped(scope: &Scope) -> Value {
    let mut fields = tallies(scope.counts, scope.estimate);
    let order = scope.order;
    if let Some(at) = order.first_spec {
        fields["firstSpec"] = json!(at.to_string());
    }
    if let Some(at) = order.first_code_edit {
        fields["firstCodeEdit"] = json!(at.to_string());
    }
    fields["codeEditsBeforeSpec"] = json!(order.code_edits_before_spec);
    if let Some(first) = order.spec_first() {
        fields["specFirst"] = json!(first);
    }
    fields
}

/// The columns of the table, each with the count it gives.
const COLUMNS: [(&str, &str); 6] = [
    ("spec", "specReads"),
    ("loft", "loftReads"),
    ("code", "codeReads"),
    ("other", "otherReads"),
    ("spec edits", "specEdits"),
    ("code edits", "codeEdits"),
];

/// The report as a table for a person.
fn table(fields: &Value) -> Vec<String> {
    let branch = fields["branch"].as_str().unwrap_or_default();
    if fields["measured"] != true {
        let mut lines = vec![format!(
            "Nothing has been measured on {branch}: no session's adapter counted what its agents consulted."
        )];
        lines.extend(unmeasured(fields));
        return lines;
    }
    let names = cells(COLUMNS.iter().map(|(name, _)| name));
    let head = format!("{:<32}{names}{:>14}", "stage", "spec first");
    let mut lines = vec![format!("What the agents consulted on {branch}"), head];
    let stages = fields["stages"].as_array().into_iter().flatten();
    for stage in stages {
        let name = stage["title"].as_str().or(stage["stage"].as_str());
        lines.push(row(name.unwrap_or_default(), stage));
    }
    if let Some(none) = fields.get("noStage") {
        lines.push(row("(no stage)", none));
    }
    lines.push(row("in all", &fields["all"]));
    for agent in fields["agents"].as_array().into_iter().flatten() {
        let kind = agent["type"].as_str().unwrap_or_default();
        lines.push(row(&format!("agent: {kind}"), agent));
    }
    let estimate = &fields["all"]["estimate"];
    let classes: Vec<String> = Estimate::NAMES
        .iter()
        .map(|name| format!("{name} {}", estimate[*name]))
        .collect();
    let partial = if fields["partial"] == true {
        ", partial"
    } else {
        ""
    };
    lines.push(format!(
        "Estimated context tokens (an estimate{partial}): {}",
        classes.join(", ")
    ));
    lines.extend(unmeasured(fields));
    lines
}

/// Each of `values` right-aligned in a cell of the table.
fn cells<T: std::fmt::Display>(values: impl Iterator<Item = T>) -> String {
    values.fold(String::new(), |mut line, value| {
        let _ = write!(line, "{value:>12}");
        line
    })
}

fn row(name: &str, scope: &Value) -> String {
    let counts = cells(COLUMNS.iter().map(|(_, count)| &scope["counts"][count]));
    let first = match scope["specFirst"].as_bool() {
        Some(true) => "yes".to_owned(),
        Some(false) => format!("no ({} before)", scope["codeEditsBeforeSpec"]),
        None => String::new(),
    };
    let line = format!("{:<32}{counts}{first:>14}", clipped(name, 31));
    line.trim_end().to_owned()
}

fn clipped(name: &str, width: usize) -> String {
    if name.chars().count() <= width {
        return name.to_owned();
    }
    let mut kept: String = name.chars().take(width - 1).collect();
    kept.push('…');
    kept
}

fn unmeasured(fields: &Value) -> Vec<String> {
    let sessions: Vec<String> = fields["unmeasured"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| {
            let session = entry["session"].as_str().unwrap_or_default();
            let host = entry["host"].as_str().unwrap_or_default();
            format!("{session} ({host})")
        })
        .collect();
    if sessions.is_empty() {
        return Vec::new();
    }
    vec![format!("Unmeasured sessions: {}", sessions.join(", "))]
}

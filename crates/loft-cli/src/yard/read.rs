//! `loft yard read` (the Read Yard Log Feature): the branch's Yard Log at its current revision,
//! or only what changed since a revision the agent read before. A read records, in
//! `~/.yard/last-read.json`, the revision that agent read; it changes no file under `.yard/` and
//! does not raise the revision, but for settling Yard Log a merge left unsettled.

use std::path::Path;

use serde_json::{Value, json};
use yardlog::{Actor, Format};
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;
use yardlog_store::{Held, Place};

use super::args::ReadArgs;
use super::branch::{Branch, revision_of};
use super::caller::{Caller, Continuations, LastRead};
use super::failure::Failure;
use super::render_log::{Selection, closed_by, root, stages, submissions};
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-read/1";
const OUTCOME: &str = "read";

pub fn run(root: &Path, args: ReadArgs) -> Finished {
    let as_json = args.output.json();
    match document(root, args) {
        Ok(fields) => {
            if as_json {
                json::print(&json::result(SCHEMA, OUTCOME, fields, Vec::new()));
            } else {
                outln!("{}", summary(&fields));
            }
            Finished::success(OUTCOME)
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}

/// The actor of a settling a read makes: the calling agent, or the person where it names none.
pub fn settler(caller: &Caller, branch: &Branch) -> Result<Actor, Failure> {
    if let Some(agent) = &caller.agent {
        return Ok(Actor::Agent(agent.clone()));
    }
    let (name, email) = super::answer::person(branch)?;
    Ok(Actor::Person { name, email })
}

/// What a read on a detached HEAD gives: Yard Log belongs to a branch, so there is none, with no
/// stage at revision 0.
fn detached(place: &Place, port: u16) -> Value {
    json!({
        "tree": place.tree.display().to_string(),
        "commonDir": place.common_dir.display().to_string(),
        "detached": true,
        "revision": 0,
        "page": { "available": false, "port": port },
        "ignored": false,
        "stages": [],
        "submissions": [],
    })
}

pub fn document(dir: &Path, args: ReadArgs) -> Result<Value, Failure> {
    let caller = Caller::new(args.agent);
    let git = YardGit::default();
    let place = Place::locate(&git, dir)?;
    if place.branch.is_none() {
        let mut fields = detached(&place, args.beyond.port);
        fields["ignored"] = json!(git.ignores_yard(&place.tree)?);
        return Ok(fields);
    }
    let branch = Branch::at(git, place, args.beyond)?;
    let home = Home::from_env();
    let at = super::now()?;
    caller.arrive(home.as_ref(), at)?;
    let held = branch.open(at, || settler(&caller, &branch))?;
    let mut fields = branch.fields(revision_of(&held));
    fields["ignored"] = json!(branch.git.ignores_yard(&branch.place().tree)?);
    if let Some(since) = args.since {
        fields["since"] = json!(since);
    }
    let (log, left_out) = match held {
        Held::Log(log) => (*log, None),
        // Yard Log of a newer format is read for what this `loft` can read of it.
        Held::Newer(_) => match branch.store.read_partial()? {
            Some(partial) => (partial.log, Some(partial.left_out)),
            None => return Ok(absent(fields, &branch)),
        },
        Held::Absent => return Ok(absent(fields, &branch)),
    };
    let log = &log;
    if let Some(left_out) = left_out {
        fields["leftOut"] = json!(left_out);
    }
    let closed_by_merge = !log.is_closed() && branch.closed_by_merge(log)?;
    fields["yardLog"] = root(log, closed_by_merge);
    let selection = Selection {
        since: args.since,
        every_version: args.versions,
    };
    fields["stages"] = Value::Array(stages(log, &selection));
    let continuations = Continuations::read(home.as_ref())?;
    fields["submissions"] = Value::Array(submissions(log, &continuations));
    if let (Some(agent), Some(home)) = (&caller.agent, &home) {
        let reader = continuations.identity(agent);
        let last_read = LastRead::new(home);
        if let Some(before) = last_read.before(log.root.key, &reader)? {
            fields["lastRead"] = json!(before);
        }
        if !args.peek {
            last_read.record(log.root.key, &reader, log.root.revision)?;
        }
    }
    Ok(fields)
}

/// What a read gives of a branch that has no Yard Log yet.
fn absent(mut fields: Value, branch: &Branch) -> Value {
    fields["yardLog"] = json!({
        "title": branch.store.branch(),
        "branch": branch.store.branch(),
        "format": Format::CURRENT.to_string(),
        "closedBy": "open",
        "merged": [],
    });
    fields["stages"] = json!([]);
    fields["submissions"] = json!([]);
    fields
}

/// One line for a person at a terminal.
fn summary(fields: &Value) -> String {
    let stages = fields["stages"].as_array().map_or(0, Vec::len);
    let closed = match fields["yardLog"]["closedBy"].as_str() {
        Some(by) if by != closed_by(false, false) => format!(", closed by the {by}"),
        _ => String::new(),
    };
    format!(
        "Yard Log of {} at revision {}: {stages} stages{closed}; {}",
        fields["branch"].as_str().unwrap_or("a detached HEAD"),
        fields["revision"],
        page_line(&fields["page"])
    )
}

/// The page's address for a person, or that it is unavailable.
pub fn page_line(page: &Value) -> String {
    match page["url"].as_str() {
        Some(url) => format!("page {url}"),
        None => format!("page unavailable (port {})", page["port"]),
    }
}

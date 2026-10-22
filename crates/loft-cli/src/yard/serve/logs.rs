//! A branch's Yard Log as the page reads it, and the person's answers and decisions as the page
//! sends them. The tree's current branch's Yard Log reads as `loft yard read` gives it, with
//! every version of each block; one carried from another branch reads the same way, marked as
//! inherited, and takes no answer or decision. Answers are recorded through the same rules as
//! `loft yard answer`, on the channel `page`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use yardlog::Channel;
use yardlog_store::Held;
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;
use yardlog_store::session::SessionFile;
use yardlog_store::{Place, YardStore};

use super::http::Response;
use super::trees::{is_served, store_of};
use super::{markdown, session_files};
use crate::cli::{Format, Output};
use crate::json;
use crate::yard::answer::record;
use crate::yard::answer_input;
use crate::yard::args::{Beyond, DEFAULT_BASE, ReadArgs, Runner};
use crate::yard::branch::Branch;
use crate::yard::caller::{Caller, Continuations};
use crate::yard::failure::Failure;
use crate::yard::read::document;
use crate::yard::render::{act, actor};
use crate::yard::render_log::{Selection, root, stages, submissions};

const ANSWER_SCHEMA: &str = "loft-yard-answer/1";
const READ_SCHEMA: &str = "loft-yard-read/1";

/// The tree a request names, where the page server serves it.
fn served_tree(home: &Home, named: Option<&str>) -> Result<PathBuf, Response> {
    let Some(named) = named.filter(|named| !named.is_empty()) else {
        return Err(Response::refused(400, "tree", "the request names no tree"));
    };
    let tree = Path::new(named)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(named));
    match is_served(home, &tree) {
        Ok(true) => Ok(tree),
        Ok(false) => Err(Response::refused(
            404,
            "tree",
            &format!("the page server serves no tree {named}"),
        )),
        Err(failure) => Err(answer_failure(failure, READ_SCHEMA)),
    }
}

/// The base branch a request on `tree` checks a merge against: the one it names, else the one
/// the tree was registered with, else the default.
fn base_for(home: &Home, tree: &Path, named: Option<&str>) -> String {
    named
        .map(ToOwned::to_owned)
        .or_else(|| super::trees::base_of(home, tree))
        .unwrap_or_else(|| DEFAULT_BASE.to_owned())
}

/// The answer to a request that failed.
fn answer_failure(failure: Failure, schema: &str) -> Response {
    match failure {
        Failure::Usage(message) => Response::refused(400, "usage", &message),
        Failure::Stopped(stop) => Response::json(409, &json::failure(schema, stop.errors)),
        Failure::Refused { fields, errors } => {
            Response::json(409, &json::result(schema, "refused", fields, errors))
        }
    }
}

/// `branch`'s Yard Log in the served tree `tree`, as the page reads it.
pub fn read(
    home: &Home,
    tree: Option<&str>,
    branch: Option<&str>,
    base: Option<&str>,
    port: u16,
) -> Response {
    let tree = match served_tree(home, tree) {
        Ok(tree) => tree,
        Err(refused) => return refused,
    };
    let place = match Place::locate(&YardGit::default(), &tree) {
        Ok(place) => place,
        Err(error) => return answer_failure(error.into(), READ_SCHEMA),
    };
    let branch = branch.or(place.branch.as_deref()).map(ToOwned::to_owned);
    let Some(branch) = branch else {
        return Response::refused(400, "branch", "the request names no branch");
    };
    let beyond = Beyond {
        base: base_for(home, &tree, base),
        port,
    };
    let read = if place.branch.as_deref() == Some(branch.as_str()) {
        current(&tree, beyond)
    } else {
        inherited(&place, &branch, port)
    };
    match read.and_then(|fields| with_sessions(fields, &place, &branch)) {
        Ok(fields) => Response::json(200, &json::result(READ_SCHEMA, "read", fields, Vec::new())),
        Err(failure) => answer_failure(failure, READ_SCHEMA),
    }
}

/// A read document with its Markdown rendered and the files its sessions keep, their activity
/// and their token totals.
fn with_sessions(mut fields: Value, place: &Place, branch: &str) -> Result<Value, Failure> {
    markdown::render_document(&mut fields);
    let store = store_of(place, branch)?;
    let folder = store.folder();
    fields["activity"] = Value::Array(session_files::read(&folder, SessionFile::Activity));
    fields["tokens"] = Value::Array(session_files::read(&folder, SessionFile::Tokens));
    if let Ok(Held::Log(log)) = store.read() {
        fields["sent"] = sent(&log);
    }
    Ok(fields)
}

/// Every submission of `log`, open or not, with its agent, its blocks and each change of its
/// state, for the page's chart of where the time went.
fn sent(log: &yardlog::YardLog) -> Value {
    let submissions = log.root.submissions.iter().map(|submission| {
        let changes: Vec<Value> = submission
            .changes
            .iter()
            .map(|change| json!({ "what": change.what.as_str(), "act": act(&change.act) }))
            .collect();
        json!({
            "key": submission.key.to_string(),
            "agent": actor(submission.sender()),
            "blocks": submission.blocks.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "state": submission.state(log).as_str(),
            "changes": changes,
        })
    });
    Value::Array(submissions.collect())
}

/// The tree's current branch's Yard Log, as `loft yard read --versions` gives it.
fn current(tree: &Path, beyond: Beyond) -> Result<Value, Failure> {
    let args = ReadArgs {
        since: None,
        versions: true,
        peek: true,
        agent: Runner {
            session: None,
            agent: None,
            agent_type: None,
            continues: None,
        },
        beyond,
        output: Output {
            format: Some(Format::Json),
        },
    };
    let mut fields = document(tree, args)?;
    fields["inherited"] = json!(false);
    Ok(fields)
}

/// Yard Log of a branch the tree is not on, carried from the branch it was cut from: shown to
/// read, never settled or written.
fn inherited(place: &Place, branch: &str, port: u16) -> Result<Value, Failure> {
    let store: YardStore = store_of(place, branch)?;
    let mut fields = json!({
        "tree": place.tree.display().to_string(),
        "branch": branch,
        "commonDir": place.common_dir.display().to_string(),
        "inherited": true,
        "page": { "available": true, "port": port, "url": crate::yard::page::url(port, &place.tree, branch) },
    });
    let log = match store.read()? {
        Held::Log(log) => *log,
        Held::Newer(_) => match store.read_partial()? {
            Some(partial) => {
                fields["leftOut"] = json!(partial.left_out);
                partial.log
            }
            None => return Err(Failure::one("absent", "the branch has no Yard Log")),
        },
        Held::Absent => return Err(Failure::one("absent", "the branch has no Yard Log")),
    };
    fields["revision"] = json!(log.root.revision);
    fields["yardLog"] = root(&log, false);
    let selection = Selection {
        since: None,
        every_version: true,
    };
    fields["stages"] = Value::Array(stages(&log, &selection));
    let continuations = Continuations::read(Home::from_env().as_ref())?;
    fields["submissions"] = Value::Array(submissions(&log, &continuations));
    Ok(fields)
}

/// Records the answers and decisions the page sent: `{ tree, branch, answers, base? }`, each
/// answer as `loft yard answer` takes it.
pub fn answer(home: &Home, body: &[u8], port: u16) -> Response {
    let Ok(sent) = serde_json::from_slice::<Value>(body) else {
        return Response::refused(400, "input", "the request's body is not JSON");
    };
    let tree = match served_tree(home, sent["tree"].as_str()) {
        Ok(tree) => tree,
        Err(refused) => return refused,
    };
    let beyond = Beyond {
        base: base_for(home, &tree, sent["base"].as_str()),
        port,
    };
    let branch = match Branch::locate(&tree, beyond) {
        Ok(branch) => branch,
        Err(failure) => return answer_failure(failure, ANSWER_SCHEMA),
    };
    if sent["branch"].as_str() != Some(branch.store.branch()) {
        return Response::refused(
            409,
            "inherited",
            "the page takes answers only for the Yard Log of the branch the tree is on",
        );
    }
    let named = match answer_input::read(&sent["answers"]) {
        Ok(named) => named,
        Err(failure) => return answer_failure(failure, ANSWER_SCHEMA),
    };
    let caller = Caller::new(Runner {
        session: None,
        agent: None,
        agent_type: None,
        continues: None,
    });
    match record(&branch, &named, &caller, Channel::Page) {
        Ok((fields, errors)) => {
            let outcome = if errors.is_empty() {
                "answered"
            } else {
                "answered elsewhere"
            };
            Response::json(200, &json::result(ANSWER_SCHEMA, outcome, fields, errors))
        }
        Err(failure) => answer_failure(failure, ANSWER_SCHEMA),
    }
}

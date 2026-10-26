//! A branch's Yard Log as the page reads it, and the person's answers and decisions as the page
//! sends them. The tree's current branch's Yard Log reads as `loft yard read` gives it, with
//! every version of each block; one carried from another branch reads the same way, marked as
//! inherited, and takes no answer or decision. Answers are recorded through the same rules as
//! `loft yard answer`, on the channel `page`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use yardlog::Channel;
use yardlog_store::Held;
use yardlog_store::Place;
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;
use yardlog_store::session::SessionFile;

use super::http::Response;
use super::trees::is_served;
use super::{markdown, session_files};
use crate::json;
use crate::yard::answer::{OUTCOME, PARTLY, record};
use crate::yard::answer_input;
use crate::yard::args::{Beyond, Runner};
use crate::yard::branch::Branch;
use crate::yard::caller::{Caller, Continuations};
use crate::yard::failure::Failure;
use crate::yard::read::{settler, shown};
use crate::yard::render::{act, actor};
use crate::yard::render_log::Selection;

const ANSWER_SCHEMA: &str = "loft-yard-answer/1";
const READ_SCHEMA: &str = "loft-yard-read/2";

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
pub fn read(home: &Home, tree: Option<&str>, branch: Option<&str>, port: u16) -> Response {
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
    let beyond = Beyond { port };
    let read = if place.branch.as_deref() == Some(branch.as_str()) {
        current(place, beyond)
    } else {
        inherited(&place, &branch, beyond)
    };
    match read {
        Ok(fields) => Response::json(200, &json::result(READ_SCHEMA, "read", fields, Vec::new())),
        Err(failure) => answer_failure(failure, READ_SCHEMA),
    }
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

/// The tree's current branch's Yard Log, as `loft yard read --versions` gives it: settled where
/// a merge left it unsettled, by the person, as a read with no agent settles it.
fn current(place: Place, beyond: Beyond) -> Result<Value, Failure> {
    let branch = Branch::at(YardGit::default(), place, beyond)?;
    let caller = Caller::new(&Runner::default());
    let held = branch.open(crate::yard::now()?, || settler(&caller, &branch))?;
    shown_with_versions(&branch, held, false)
}

/// Yard Log of a branch the tree is not on, carried from the branch it was cut from: rendered
/// as the current branch's is, never settled or written.
fn inherited(place: &Place, branch: &str, beyond: Beyond) -> Result<Value, Failure> {
    let branch = Branch::at(YardGit::default(), place.on(branch), beyond)?;
    let held = branch.store.read()?;
    if matches!(held, Held::Absent) {
        return Err(Failure::one("absent", "the branch has no Yard Log"));
    }
    shown_with_versions(&branch, held, true)
}

/// Yard Log as a read with every version renders it, recording nothing, with its Markdown
/// rendered, every submission it sent, the files its sessions keep, their activity and their
/// token totals, and whether it is `inherited` from the branch the tree's was cut from.
fn shown_with_versions(branch: &Branch, held: Held, inherited: bool) -> Result<Value, Failure> {
    let selection = Selection {
        since: None,
        every_version: true,
    };
    let continuations = Continuations::read(Home::from_env().as_ref())?;
    let sent = match &held {
        Held::Log(log) => Some(sent(log)),
        Held::Absent | Held::Newer(_) => None,
    };
    let mut fields = shown(branch, held, &selection, &continuations)?.0;
    markdown::render_document(&mut fields);
    let folder = branch.store.folder();
    fields["activity"] = Value::Array(session_files::read(&folder, SessionFile::Activity));
    fields["tokens"] = Value::Array(session_files::read(&folder, SessionFile::Tokens));
    if let Some(sent) = sent {
        fields["sent"] = sent;
    }
    fields["inherited"] = json!(inherited);
    Ok(fields)
}

/// Records the answers and decisions the page sent: `{ tree, branch, answers }`, each
/// answer as `loft yard answer` takes it.
pub fn answer(home: &Home, body: &[u8], port: u16) -> Response {
    let Ok(sent) = serde_json::from_slice::<Value>(body) else {
        return Response::refused(400, "input", "the request's body is not JSON");
    };
    let tree = match served_tree(home, sent["tree"].as_str()) {
        Ok(tree) => tree,
        Err(refused) => return refused,
    };
    let beyond = Beyond { port };
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
    match record(&branch, &named, Channel::Page) {
        Ok((fields, errors)) => {
            let outcome = if errors.is_empty() { OUTCOME } else { PARTLY };
            Response::json(200, &json::result(ANSWER_SCHEMA, outcome, fields, errors))
        }
        Err(failure) => answer_failure(failure, ANSWER_SCHEMA),
    }
}

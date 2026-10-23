//! `loft yard published` (the Record a publish after the merge Feature): on the base branch, once
//! `loft publish` is committed, records the `publish` view of the tree in the Yard Log of the
//! branch the merge brought in, in its last stage, after its close. It is refused on another
//! branch, for Yard Log the base branch does not hold, and for Yard Log that holds such a publish
//! already; a refusal changes no file.

use std::path::Path;

use serde_json::{Value, json};
use yardlog::body::{Loft, View};
use yardlog::input::LoftRequest;
use yardlog::published::Unpublished;
use yardlog_store::git::YardGit;
use yardlog_store::{Held, Place};

use super::args::PublishedArgs;
use super::branch::{Branch, Unapplied};
use super::caller::Caller;
use super::failure::{self, Failure};
use super::views::Views;
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-published/1";
const OUTCOME: &str = "recorded";

pub fn run(root: &Path, args: PublishedArgs) -> Finished {
    let as_json = args.output.json();
    match attempt(root, args) {
        Ok(fields) => {
            if as_json {
                json::print(&json::result(SCHEMA, OUTCOME, fields, Vec::new()));
            } else {
                outln!(
                    "Recorded the publish in the Yard Log of {} at revision {}",
                    fields["branch"].as_str().unwrap_or_default(),
                    fields["revision"]
                );
            }
            Finished::success(OUTCOME)
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}

fn attempt(dir: &Path, args: PublishedArgs) -> Result<Value, Failure> {
    let caller = Caller::new(&args.writer.into());
    let git = YardGit::default();
    let place = Place::locate(&git, dir)?;
    let at = super::now()?;
    let on = place.branch()?.to_owned();
    let named = args.branch;
    let branch = Branch::at(git, place.on(&named), args.beyond)?;
    let held = branch.store.read()?;
    // The base branch is the one the named Yard Log records; a tree that holds none of it has
    // none to name.
    let base = match &held {
        Held::Log(log) => Some(log.root.base.clone()),
        Held::Newer(_) => branch
            .store
            .read_partial()?
            .map(|partial| partial.log.root.base),
        Held::Absent => None,
    };
    let looked_for = match &base {
        Some(base) => format!("run on {on}, the base branch {base}, for the Yard Log of {named}"),
        None => format!("run on {on}, for the Yard Log of {named}"),
    };
    let absent = || {
        let message = format!("the base branch holds no such Yard Log: {looked_for}");
        failure::error("absent", &message)
    };
    let Some(base) = base else {
        return Err(refused(&branch, &held, absent()));
    };
    if on != base {
        let message = format!("a publish is recorded on the base branch only: {looked_for}");
        let error = failure::error("not-base", &message);
        return Err(refused(&branch, &Held::Absent, error));
    }
    let views = Views {
        tree: dir,
        base: &base,
        at,
    };
    let loft = publish_view(&views, args.note).map_err(|error| refused(&branch, &held, error))?;
    let agent = caller.agent().clone();
    let applied = branch.update(|current| {
        let mut log = current.ok_or_else(|| Unapplied::Refused(absent()))?;
        let recorded = log
            .record_published(loft, agent, at)
            .map_err(|refused| Unapplied::Refused(unpublished(&refused)))?;
        Ok((log, recorded))
    })?;
    match applied {
        Ok((id, key, revision)) => {
            let mut fields = branch.fields(revision);
            fields["block"] = json!({ "id": id.as_str(), "key": key.to_string() });
            Ok(fields)
        }
        Err(error) => Err(refused(&branch, &branch.store.read()?, error)),
    }
}

/// The `publish` view of the tree as the block records it, with the agent's note; or, where it
/// cannot be computed, the error that says why.
fn publish_view(views: &Views<'_>, note: Option<String>) -> Result<Loft, Value> {
    let request = LoftRequest {
        view: View::Publish,
        note: note.clone(),
        base: None,
        decision: None,
        reasons: Vec::new(),
    };
    let (recorded, report) = views.report(&request).map_err(|error| {
        let message = format!("the `publish` view could not be computed: {error}");
        failure::error("view", &message)
    })?;
    Ok(Loft {
        view: View::Publish,
        note,
        base: None,
        decision: None,
        reasons: Vec::new(),
        recorded,
        report,
    })
}

/// The error a refused record reports.
fn unpublished(refused: &Unpublished) -> Value {
    let kind = match refused {
        Unpublished::Recorded(_) => "recorded",
        Unpublished::NoStage => "no-stage",
        Unpublished::NotPublish => "view",
    };
    failure::error(kind, &refused.to_string())
}

fn refused(branch: &Branch, held: &Held, error: Value) -> Failure {
    branch.refused(held, vec![error])
}

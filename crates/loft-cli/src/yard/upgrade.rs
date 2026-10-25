//! `loft yard upgrade` (the Upgrade open Yard Logs Feature): every branch's Yard Log under the
//! tree's `.yard/` that is open and in an older format, written in this `loft`'s format, each
//! under its own lock, whole or not at all. Closed Yard Log and Yard Log already in this
//! format are left as they are; one of a newer format, or one that does not read, is left and
//! named, and the command exits 1. The person runs it, so it takes no `--session`.

use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};
use yardlog::{Actor, Format};
use yardlog_store::git::YardGit;
use yardlog_store::session::branches;
use yardlog_store::{Held, Place, YardStoreError};

use super::args::UpgradeArgs;
use super::branch::{Branch, Unapplied};
use super::failure::Failure;
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-upgrade/1";
const OUTCOME: &str = "upgraded";
/// Some Yard Log was left for a newer `loft`, or did not read.
const LEFT: &str = "left some";

/// What became of one branch's Yard Log.
enum Fate {
    Upgraded {
        key: String,
        from: Format,
    },
    Current {
        key: String,
    },
    Left {
        key: Option<String>,
        reason: String,
        fails: bool,
    },
}

pub fn run(root: &Path, args: &UpgradeArgs) -> Finished {
    let as_json = args.output.json();
    let fates = match attempt(root, args) {
        Ok(fates) => fates,
        Err(failure) => return failure.finish(SCHEMA, as_json),
    };
    let (fields, errors) = result(&fates);
    let failed = !errors.is_empty();
    let outcome = if failed { LEFT } else { OUTCOME };
    let code = if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    };
    let finished = Finished::with(code, outcome, &errors);
    if as_json {
        json::print(&json::result(SCHEMA, outcome, fields, errors));
    } else {
        outln!(
            "Upgraded {} Yard Logs; {} already current; {} left",
            fields["upgraded"].as_array().map_or(0, Vec::len),
            fields["current"].as_array().map_or(0, Vec::len),
            fields["left"].as_array().map_or(0, Vec::len),
        );
    }
    finished
}

fn attempt(dir: &Path, args: &UpgradeArgs) -> Result<Vec<(String, Fate)>, Failure> {
    let git = YardGit::default();
    let place = Place::locate(&git, dir)?;
    let mut fates = Vec::new();
    for name in branches(&place.tree)? {
        let branch = Branch::at(YardGit::default(), place.on(&name), args.beyond.clone())?;
        fates.push((name, fate(&branch)?));
    }
    Ok(fates)
}

fn fate(branch: &Branch) -> Result<Fate, Failure> {
    let left = |key: Option<String>, reason: String, fails: bool| Fate::Left { key, reason, fails };
    let log = match branch.store.read() {
        Ok(Held::Log(log)) => log,
        Ok(Held::Newer(format)) => {
            let reason = format!("written in {format}, newer than this `loft` writes");
            return Ok(left(None, reason, true));
        }
        Ok(Held::Absent) => return Ok(left(None, "has no root".to_owned(), true)),
        Err(error @ YardStoreError::Unreadable { .. }) => {
            return Ok(left(None, error.to_string(), true));
        }
        Err(error) => return Err(error.into()),
    };
    let key = log.root.key.to_string();
    if log.is_closed() || branch.closed_by_merge(&log)? {
        return Ok(left(Some(key), "closed".to_owned(), false));
    }
    if log.root.format >= Format::CURRENT {
        return Ok(Fate::Current { key });
    }
    let (name, email) = super::answer::person(branch)?;
    let actor = Actor::Person { name, email };
    let at = super::now()?;
    let upgraded = branch.update(|current| {
        // Gone since it was read: nothing is written in its place.
        let mut log = current.ok_or(Unapplied::Refused(()))?;
        let from = log.upgrade(at, actor);
        Ok((log, from))
    })?;
    Ok(match upgraded {
        Ok(Some(from)) => Fate::Upgraded { key, from },
        Ok(None) => Fate::Current { key },
        Err(()) => left(Some(key), "has no root".to_owned(), true),
    })
}

fn result(fates: &[(String, Fate)]) -> (Value, Vec<Value>) {
    let (mut upgraded, mut current, mut left, mut errors) = (vec![], vec![], vec![], vec![]);
    for (branch, fate) in fates {
        match fate {
            Fate::Upgraded { key, from } => {
                upgraded.push(json!({ "branch": branch, "key": key, "from": from.to_string() }));
            }
            Fate::Current { key } => current.push(json!({ "branch": branch, "key": key })),
            Fate::Left { key, reason, fails } => {
                let mut entry = json!({ "branch": branch, "reason": reason });
                if let Some(key) = key {
                    entry["key"] = json!(key);
                }
                left.push(entry);
                if *fails {
                    let message = format!("the Yard Log of `{branch}` is left: {reason}");
                    errors.push(super::failure::error("left", &message));
                }
            }
        }
    }
    let fields = json!({ "upgraded": upgraded, "current": current, "left": left });
    (fields, errors)
}

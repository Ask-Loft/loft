//! The trees the page server serves, kept in `~/.yard/trees.json` so that a page server started
//! later serves them again; and what the page lists of them: each repository, by the top of its
//! main worktree, each of its trees served, and each branch's Yard Log under the tree's `.yard/`,
//! the tree's current branch's first and those carried from another branch after it, offered
//! only to read. Above the list come the open submissions of every current branch's Yard Log.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use yardlog::YardLog;
use yardlog::body::Body;
use yardlog_store::git::YardGit;
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::session::branches;
use yardlog_store::{Held, Place, YardStore};

use crate::yard::failure::Failure;
use crate::yard::home_json;
use crate::yard::page;
use crate::yard::render::{act, actor};

/// What `~/.yard/trees.json` holds: the trees served.
#[derive(Default)]
struct Registry {
    trees: Vec<String>,
}

impl Registry {
    fn read(bytes: Option<&[u8]>) -> Self {
        let file = bytes
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
            .unwrap_or(Value::Null);
        let trees = file["trees"].as_array().map_or_else(Vec::new, |trees| {
            trees
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        });
        Self { trees }
    }

    fn bytes(&self) -> Vec<u8> {
        home_json::bytes(&json!({ "trees": self.trees }))
    }
}

/// Adds the worktree that holds `dir` to the trees served, once; drops the trees that no longer
/// exist; gives its top.
pub fn register(home: &Home, dir: &Path) -> Result<PathBuf, Failure> {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_owned());
    let tree = YardGit::default().top_level(&dir)?;
    let shown = tree.display().to_string();
    home.update(HomeFile::Trees, |bytes| {
        let mut registry = Registry::read(bytes.as_deref());
        let before = registry.bytes();
        registry.trees.retain(|tree| Path::new(tree).is_dir());
        if !registry.trees.contains(&shown) {
            registry.trees.push(shown.clone());
            registry.trees.sort();
        }
        let after = registry.bytes();
        (after != before).then_some(after)
    })?;
    Ok(tree)
}

/// The trees served that still exist. The file is read without its lock: it is only ever
/// replaced whole, by a rename, so a read sees one whole version of it.
pub fn served(home: &Home) -> Result<Vec<PathBuf>, Failure> {
    let registry = Registry::read(home.read(HomeFile::Trees)?.as_deref());
    Ok(registry
        .trees
        .into_iter()
        .map(PathBuf::from)
        .filter(|tree| tree.is_dir())
        .collect())
}

/// Whether `tree` is one the page server serves.
pub fn is_served(home: &Home, tree: &Path) -> Result<bool, Failure> {
    Ok(served(home)?.iter().any(|served| served == tree))
}

/// The repository a worktree belongs to, named by the top of its main worktree.
pub fn repository(place: &Place) -> String {
    let common = &place.common_dir;
    let top = if common.file_name().is_some_and(|name| name == ".git") {
        common.parent().unwrap_or(common)
    } else {
        common
    };
    top.display().to_string()
}

/// The store of `branch`'s Yard Log in the worktree `place` names, whatever branch it is on.
pub fn store_of(place: &Place, branch: &str) -> Result<YardStore, Failure> {
    let mut place = place.clone();
    place.branch = Some(branch.to_owned());
    Ok(YardStore::new(place)?)
}

/// The blocks of `log`'s open submissions that wait on the person: those not yet answered or
/// decided.
pub fn waiting(log: &YardLog) -> usize {
    open(log).map(|(_, blocks)| blocks.len()).sum()
}

/// Each open submission of `log`, with the ids of its blocks that wait on the person.
fn open(log: &YardLog) -> impl Iterator<Item = (&yardlog::log::Submission, Vec<String>)> {
    log.root
        .submissions
        .iter()
        .filter(|submission| submission.state(log) == yardlog::query::State::Open)
        .map(|submission| {
            let blocks = submission
                .blocks
                .iter()
                .filter_map(|key| log.block(*key))
                .filter(|(_, block)| block.counted().is_none())
                .map(|(_, block)| block.id.to_string())
                .collect();
            (submission, blocks)
        })
}

/// What the page lists of every tree served: by repository, then by tree; and above them, the
/// open submissions of each current branch's Yard Log, oldest first.
pub fn listing(home: &Home, port: u16) -> Result<Value, Failure> {
    let git = YardGit::default();
    let mut repositories: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut waits = Vec::new();
    for tree in served(home)? {
        let Ok(place) = Place::locate(&git, &tree) else {
            continue;
        };
        let repository = repository(&place);
        let mut logs = Vec::new();
        let mut found = branches(&place.tree)?;
        let current = place.branch.as_ref();
        if let Some(at) = found.iter().position(|branch| Some(branch) == current) {
            let current = found.remove(at);
            found.insert(0, current);
        }
        for branch in found {
            let inherited = place.branch.as_deref() != Some(branch.as_str());
            let summary = summary(&place, &branch, inherited, port, &repository, &mut waits);
            logs.push(summary);
        }
        repositories.entry(repository).or_default().push(json!({
            "tree": place.tree.display().to_string(),
            "branch": place.branch,
            "logs": logs,
        }));
    }
    waits.sort_by(|a: &Value, b: &Value| {
        let at = |wait: &Value| wait["sent"]["at"].as_str().unwrap_or_default().to_owned();
        at(a).cmp(&at(b))
    });
    let count: usize = waits
        .iter()
        .map(|wait| wait["blocks"].as_array().map_or(0, Vec::len))
        .sum();
    let repositories: Vec<Value> = repositories
        .into_iter()
        .map(|(repository, trees)| json!({ "repository": repository, "trees": trees }))
        .collect();
    Ok(json!({ "repositories": repositories, "waiting": waits, "count": count }))
}

/// What the page lists of `branch`'s Yard Log in the tree: its title, its revision, whether it
/// is closed, inherited or of a newer format, and how many of its blocks wait on the person;
/// the open submissions of a current branch's Yard Log are added to `waits`.
fn summary(
    place: &Place,
    branch: &str,
    inherited: bool,
    port: u16,
    repository: &str,
    waits: &mut Vec<Value>,
) -> Value {
    let mut fields = json!({
        "branch": branch,
        "inherited": inherited,
        "url": page::url(port, &place.tree, branch),
    });
    let held = store_of(place, branch).and_then(|store| Ok(store.read()?));
    match held {
        Ok(Held::Log(log)) => {
            fields["key"] = json!(log.root.key.to_string());
            fields["title"] = json!(log.root.title);
            fields["revision"] = json!(log.root.revision);
            fields["format"] = json!(log.root.format.to_string());
            fields["closed"] = json!(log.is_closed());
            fields["waiting"] = json!(if inherited || log.is_closed() {
                0
            } else {
                waiting(&log)
            });
            if !inherited && !log.is_closed() {
                for (submission, blocks) in open(&log) {
                    let asked: Vec<Value> = blocks.iter().map(|id| asking(&log, id)).collect();
                    waits.push(json!({
                        "asked": asked,
                        "repository": repository,
                        "tree": place.tree.display().to_string(),
                        "branch": branch,
                        "url": page::url(port, &place.tree, branch),
                        "submission": submission.key.to_string(),
                        "sent": act(submission.sending()),
                        "agent": actor(submission.sender()),
                        "blocks": blocks,
                    }));
                }
            }
        }
        Ok(Held::Newer(format)) => {
            fields["format"] = json!(format.to_string());
            fields["newer"] = json!(true);
            fields["waiting"] = json!(0);
        }
        Ok(Held::Absent) => fields["waiting"] = json!(0),
        Err(failure) => {
            fields["error"] = failure_value(failure);
            fields["waiting"] = json!(0);
        }
    }
    fields
}

/// What the block `id` of `log` asks the person: its id, its kind, and its question's prompt or
/// its approval's or plan's title.
fn asking(log: &YardLog, id: &str) -> Value {
    let found = log
        .stages
        .iter()
        .flat_map(|file| &file.blocks)
        .find(|block| block.id.as_str() == id);
    let Some(block) = found else {
        return json!({ "id": id });
    };
    let text = match &block.current().body {
        Body::Question(question) => question.prompt.clone(),
        Body::Approval(approval) => approval.title.clone(),
        Body::Plan(plan) => plan.title.clone(),
        _ => String::new(),
    };
    json!({ "id": id, "kind": block.kind.as_str(), "text": text })
}

/// The errors a failure gives, for a listing that goes on past it.
pub fn failure_value(failure: Failure) -> Value {
    match failure {
        Failure::Stopped(stop) => json!(stop.errors),
        Failure::Refused { errors, .. } => json!(errors),
        Failure::Usage(message) => json!([crate::json::error("usage", &message, &[])]),
    }
}

//! The trees the page server serves, kept in `~/.yard/trees.json` so that a page server started
//! later serves them again; and what the page lists of them: each repository, by the top of its
//! main worktree, each of its trees served, and each branch's Yard Log under the tree's `.yard/`,
//! the tree's current branch's first and those carried from another branch after it, offered
//! only to read. Above the list come the open submissions of every current branch's Yard Log.
//! What the list says of a Yard Log is kept with the stamp of its root file, which each write to
//! the Yard Log rewrites, and read again only once that stamp moves.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::{Value, json};
use yardlog::YardLog;
use yardlog::body::Body;
use yardlog::codec::ROOT;
use yardlog::log::{Block, Submission};
use yardlog_store::git::YardGit;
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::session::branches;
use yardlog_store::{Held, Place, YARD, YardStore};

use super::inherited::{Inherited, distinct, distinct_count, page};
use super::watch::{FileStamp, stamp_of};
use crate::yard::event::strings;
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
        Self {
            trees: strings(&file["trees"]),
        }
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
    add(home, &tree)?;
    Ok(tree)
}

/// Adds the top of a worktree, `tree`, to the trees served, once; drops the trees that no longer
/// exist; writes nothing where that changes nothing.
pub fn add(home: &Home, tree: &Path) -> Result<(), Failure> {
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
    Ok(())
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

/// Whether the worktree is its repository's main worktree: its own git directory, which holds
/// `yard`, is the repository's common directory.
fn is_main(place: &Place) -> bool {
    place.git_yard.parent() == Some(place.common_dir.as_path())
}

/// The store of `branch`'s Yard Log in the worktree `place` names, whatever branch it is on.
pub fn store_of(place: &Place, branch: &str) -> Result<YardStore, Failure> {
    Ok(YardStore::new(place.on(branch))?)
}

/// Each open submission of `log`, with its blocks that wait on the person: those not yet
/// answered or decided.
fn open(log: &YardLog) -> Vec<(&Submission, Vec<&Block>)> {
    log.open_submissions()
        .into_iter()
        .filter_map(|key| log.submission(key))
        .map(|submission| {
            let blocks = submission
                .blocks
                .iter()
                .filter_map(|key| log.block(*key))
                .map(|(_, block)| block)
                .filter(|block| block.counted(log).is_none())
                .collect();
            (submission, blocks)
        })
        .collect()
}

/// What the list said of one Yard Log: its entry and the open submissions it added, as they were
/// when its root file had `stamp`, read as inherited or not.
struct Entry {
    stamp: FileStamp,
    inherited: bool,
    summary: Value,
    waits: Vec<Value>,
}

/// What the page server last read of each Yard Log for the list, by its tree and its branch.
#[derive(Default)]
pub struct Entries(Mutex<HashMap<(PathBuf, String), Entry>>);

impl Entries {
    /// What is kept, whichever thread held it last, as a thread that panicked left it.
    fn kept(&self) -> MutexGuard<'_, HashMap<(PathBuf, String), Entry>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The entry of `branch`'s Yard Log in `place`'s tree and the open submissions it adds: the
    /// one kept while its root file keeps its stamp, or read again. One that could not be read
    /// is not kept, so that the next list tries again.
    fn entry(
        &self,
        place: &Place,
        branch: &str,
        inherited: bool,
        read: impl FnOnce() -> (Value, Vec<Value>),
    ) -> (Value, Vec<Value>) {
        let key = (place.tree.clone(), branch.to_owned());
        let root = place.tree.join(YARD).join(branch).join(ROOT);
        let stamp = fs::metadata(root).ok().map(|meta| stamp_of(&meta));
        let mut kept = self.kept();
        if let (Some(stamp), Some(entry)) = (stamp, kept.get(&key))
            && entry.stamp == stamp
            && entry.inherited == inherited
        {
            return (entry.summary.clone(), entry.waits.clone());
        }
        drop(kept);
        let (summary, waits) = read();
        kept = self.kept();
        match stamp {
            Some(stamp) if summary.get("error").is_none() => {
                let entry = Entry {
                    stamp,
                    inherited,
                    summary: summary.clone(),
                    waits: waits.clone(),
                };
                kept.insert(key, entry);
            }
            _ => {
                kept.remove(&key);
            }
        }
        (summary, waits)
    }

    /// Forgets what was read of each Yard Log that `seen` does not name.
    fn keep_only(&self, seen: &HashSet<(PathBuf, String)>) {
        let mut kept = self.kept();
        kept.retain(|key, _| seen.contains(key));
    }
}

/// What the trees served hold of one repository: each tree with its current branch's Yard Log,
/// every inherited Yard Log each of them holds, and the keys of the Yard Logs its trees are on.
#[derive(Default)]
struct Holdings {
    trees: Vec<Value>,
    inherited: Vec<Inherited>,
    current: HashSet<String>,
}

/// What every tree served holds, by repository, and the open submissions of the current
/// branches' Yard Logs.
type Gathered = (BTreeMap<String, Holdings>, Vec<Value>);

/// What `Gathered` holds, as the trees served give it now.
fn gather(home: &Home, port: u16, entries: &Entries) -> Result<Gathered, Failure> {
    let git = YardGit::default();
    let mut repositories: BTreeMap<String, Holdings> = BTreeMap::new();
    let mut waits = Vec::new();
    let mut seen = HashSet::new();
    for tree in served(home)? {
        let Ok(place) = Place::locate(&git, &tree) else {
            continue;
        };
        let repository = repository(&place);
        let held = repositories.entry(repository.clone()).or_default();
        let mut logs = Vec::new();
        for branch in branches(&place.tree)? {
            let inherited = place.branch.as_deref() != Some(branch.as_str());
            let (summary, added) = entries.entry(&place, &branch, inherited, || {
                summary(&place, &branch, inherited, port, &repository)
            });
            // A Yard Log that could not be read has no key: its tree and branch stand for it.
            let key = summary["key"].as_str().map_or_else(
                || format!("{}\n{branch}", place.tree.display()),
                str::to_owned,
            );
            seen.insert((place.tree.clone(), branch));
            waits.extend(added);
            if inherited {
                let mut entry = summary;
                entry["tree"] = json!(place.tree.display().to_string());
                let main = is_main(&place);
                held.inherited.push(Inherited { key, main, entry });
            } else {
                held.current.insert(key);
                logs.push(summary);
            }
        }
        held.trees.push(json!({
            "tree": place.tree.display().to_string(),
            "main": is_main(&place),
            "branch": place.branch,
            "logs": logs,
        }));
    }
    entries.keep_only(&seen);
    Ok((repositories, waits))
}

/// What the page lists of every tree served: by repository, then by tree with its current
/// branch's Yard Log, and the count of the repository's inherited Yard Logs; and above them, the
/// open submissions of each current branch's Yard Log, oldest first.
pub fn listing(home: &Home, port: u16, entries: &Entries) -> Result<Value, Failure> {
    let (repositories, mut waits) = gather(home, port, entries)?;
    order_waiting(&mut waits);
    let count: usize = waits
        .iter()
        .map(|wait| wait["blocks"].as_array().map_or(0, Vec::len))
        .sum();
    let repositories: Vec<Value> = repositories
        .into_iter()
        .map(|(repository, held)| {
            let inherited = distinct_count(&held.inherited, &held.current);
            json!({ "repository": repository, "trees": held.trees, "inherited": inherited })
        })
        .collect();
    Ok(json!({ "repositories": repositories, "waiting": waits, "count": count }))
}

fn order_waiting(waits: &mut [Value]) {
    // Revisions are local to one log: group equal-time ties by log before comparing them.
    waits.sort_by_cached_key(|wait| {
        let text = |field| wait[field].as_str().unwrap_or_default().to_owned();
        (
            wait["sent"]["at"].as_str().unwrap_or_default().to_owned(),
            text("tree"),
            text("branch"),
            wait["sentReached"]
                .as_u64()
                .or_else(|| wait["sent"]["revision"].as_u64())
                .unwrap_or(0),
            text("submission"),
        )
    });
}

/// The page from `from` of `repository`'s inherited Yard Logs whose branch or title holds
/// `filter`, with how many there are.
pub fn inherited(
    home: &Home,
    port: u16,
    entries: &Entries,
    repository: &str,
    filter: &str,
    from: usize,
) -> Result<Value, Failure> {
    let (mut repositories, _) = gather(home, port, entries)?;
    let held = repositories.remove(repository).unwrap_or_default();
    let mut page = page(&distinct(held.inherited, &held.current), filter, from);
    page["repository"] = json!(repository);
    Ok(page)
}

/// What the page lists of `branch`'s Yard Log in the tree: its title, its revision, whether it
/// is closed, inherited or of a newer format, and how many of its blocks wait on the person;
/// and the open submissions of a current branch's Yard Log.
fn summary(
    place: &Place,
    branch: &str,
    inherited: bool,
    port: u16,
    repository: &str,
) -> (Value, Vec<Value>) {
    let mut waits = Vec::new();
    let fields = fields(place, branch, inherited, port, repository, &mut waits);
    (fields, waits)
}

/// The fields of `summary`'s entry, its open submissions added to `waits`.
fn fields(
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
        "waiting": 0,
    });
    // Only the root is read of Yard Log that cannot wait on the person: inherited or closed.
    let read = store_of(place, branch).and_then(|store| Ok((store.read_root()?, store)));
    let (root, store) = match read {
        Ok((Some(Ok(root)), store)) => (root, store),
        Ok((Some(Err(format)), _)) => {
            fields["format"] = json!(format.to_string());
            fields["newer"] = json!(true);
            return fields;
        }
        Ok((None, _)) => return fields,
        Err(failure) => {
            fields["error"] = failure_value(failure);
            return fields;
        }
    };
    fields["key"] = json!(root.key.to_string());
    fields["title"] = json!(root.title);
    fields["revision"] = json!(root.revision);
    fields["format"] = json!(root.format.to_string());
    fields["closed"] = json!(root.closed.is_some());
    fields["openedAt"] = json!(root.opened.at.to_string());
    if let Some(closed) = &root.closed {
        fields["closedAt"] = json!(closed.at.to_string());
    }
    if inherited || root.closed.is_some() {
        return fields;
    }
    let log = match store.read() {
        Ok(Held::Log(log)) => log,
        Ok(_) => return fields,
        Err(error) => {
            fields["error"] = failure_value(error.into());
            return fields;
        }
    };
    let mut count = 0;
    for (submission, blocks) in open(&log) {
        count += blocks.len();
        let asked: Vec<Value> = blocks.iter().map(|block| asking(block)).collect();
        let ids: Vec<&str> = blocks.iter().map(|block| block.id.as_str()).collect();
        waits.push(json!({
            "asked": asked,
            "repository": repository,
            "tree": place.tree.display().to_string(),
            "main": is_main(place),
            "branch": branch,
            "url": page::url(port, &place.tree, branch),
            "submission": submission.key.to_string(),
            "sent": act(submission.sending()),
            "sentReached": crate::yard::render_log::sent_reached(submission),
            "agent": actor(submission.sender()),
            "blocks": ids,
        }));
    }
    fields["waiting"] = json!(count);
    fields
}

/// What `block` asks the person: its id, its kind, and its question's prompt or its approval's
/// or plan's title.
fn asking(block: &Block) -> Value {
    let text = match &block.current().body {
        Body::Question(question) => question.prompt.clone(),
        Body::Approval(approval) => approval.title.clone(),
        Body::Plan(plan) => plan.title.clone(),
        _ => String::new(),
    };
    json!({ "id": block.id.as_str(), "kind": block.kind.as_str(), "text": text })
}

/// The errors a failure gives, for a listing that goes on past it.
pub fn failure_value(failure: Failure) -> Value {
    match failure {
        Failure::Stopped(stop) => json!(stop.errors),
        Failure::Refused { errors, .. } => json!(errors),
        Failure::Usage(message) => json!([crate::json::error("usage", &message, &[])]),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use serde_json::json;

    use super::{Entries, Entry};

    #[test]
    fn same_second_waiting_submissions_follow_local_arrival_within_each_log() {
        let wait = |tree, key, original, arrived| {
            json!({
                "tree": tree, "branch":"feat/shelf", "submission":key,
                "sent":{"at":"2026-10-22T09:30:00Z","revision":original}, "sentReached":arrived,
            })
        };
        let mut waits = vec![
            wait("/a", "later", 1, 9),
            wait("/b", "other", 1, 1),
            wait("/a", "earlier", 80, 7),
        ];
        super::order_waiting(&mut waits);
        assert_eq!(
            waits
                .iter()
                .map(|w| w["submission"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["earlier", "later", "other"]
        );
    }

    #[test]
    fn the_page_server_forgets_what_it_read_of_a_yard_log_no_tree_holds_any_more() {
        let entries = Entries::default();
        let key = |tree: &str, branch: &str| (PathBuf::from(tree), branch.to_owned());
        for (tree, branch) in [("/work/a", "main"), ("/work/a", "old"), ("/work/b", "main")] {
            let entry = Entry {
                stamp: (1, None),
                inherited: false,
                summary: json!({}),
                waits: Vec::new(),
            };
            entries.0.lock().unwrap().insert(key(tree, branch), entry);
        }
        entries.keep_only(&HashSet::from([key("/work/a", "main")]));
        let kept: Vec<_> = entries.0.lock().unwrap().keys().cloned().collect();
        assert_eq!(kept, [key("/work/a", "main")]);
    }
}

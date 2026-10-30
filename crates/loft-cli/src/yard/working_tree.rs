//! An agent's working tree (the Feature "Follow the work into another worktree"): the worktree
//! whose Yard Log holds the agent's working stage, among the worktrees its session wrote Yard
//! Log in, else the agent's own directory. Each write of an agent records its worktree in
//! `~/.yard/session-trees.json`, under the first session of its chain, with the time of the
//! write and its place in the order of the machine's writes, which says which was last where
//! two fall in one second.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use yardlog::{Agent, Time, YardLog};
use yardlog_store::git::YardGit;
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::{Held, Place, YardStore};

use super::caller::{Continuations, Identity};
use super::failure::Failure;
use super::home_json;

/// Records that `agent` wrote Yard Log in the worktree `tree` at `at`, dropping the worktrees
/// that no longer exist.
pub fn record(home: &Home, agent: &Agent, tree: &Path, at: Time) -> Result<(), Failure> {
    let identity = Continuations::read(Some(home))?.identity(agent);
    let tree = tree.display().to_string();
    home.update(HomeFile::SessionTrees, |held| {
        let mut all = home_json::object(held.as_deref());
        let order = last_order(&all) + 1;
        prune(&mut all);
        let writers = object_at(object_at(&mut all, &identity.session_key()), &tree);
        writers.insert(
            identity.within().to_owned(),
            json!({ "at": at.to_string(), "order": order }),
        );
        Some(home_json::ordered(&all))
    })?;
    Ok(())
}

/// The latest place in the order of writes that `all` records.
fn last_order(all: &Map<String, Value>) -> u64 {
    all.values()
        .filter_map(Value::as_object)
        .flat_map(Map::values)
        .filter_map(Value::as_object)
        .flat_map(Map::values)
        .filter_map(|write| write["order"].as_u64())
        .max()
        .unwrap_or(0)
}

/// Drops each worktree that no longer exists, and each session left with none.
fn prune(all: &mut Map<String, Value>) {
    for trees in all.values_mut().filter_map(Value::as_object_mut) {
        trees.retain(|tree, _| Path::new(tree).is_dir());
    }
    all.retain(|_, trees| trees.as_object().is_some_and(|trees| !trees.is_empty()));
}

/// The object `map` holds under `name`, made empty where it holds none.
fn object_at<'a>(map: &'a mut Map<String, Value>, name: &str) -> &'a mut Map<String, Value> {
    let value = map.entry(name).or_insert_with(|| json!({}));
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("made an object")
}

/// When an agent and its session last wrote in a worktree, by the order of writes; a later one
/// compares greater, and any write of the agent's own greater than none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Last {
    pub agent: Option<u64>,
    pub session: u64,
}

/// A worktree the session wrote Yard Log in, with its open Yard Log, and when it last wrote
/// there.
pub struct Written {
    pub tree: PathBuf,
    pub log: YardLog,
    pub last: Last,
}

/// The open Yard Logs of the worktrees `agent`'s session wrote in, each where it still exists
/// and is on a branch.
pub fn written(home: &Home, agent: &Agent) -> Result<Vec<Written>, Failure> {
    let identity = Continuations::read(Some(home))?.identity(agent);
    let all = home_json::object(home.read(HomeFile::SessionTrees)?.as_deref());
    let Some(trees) = all.get(&identity.session_key()).and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    Ok(trees
        .iter()
        .filter_map(|(tree, writers)| open(Path::new(tree), writers, &identity))
        .collect())
}

/// The worktree `tree`'s open Yard Log, as `writers` record the writes there.
fn open(tree: &Path, writers: &Value, identity: &Identity) -> Option<Written> {
    if !tree.is_dir() {
        return None;
    }
    let place = Place::locate(&YardGit::default(), tree).ok()?;
    place.branch.as_ref()?;
    let top = place.tree.clone();
    let Ok(Held::Log(log)) = YardStore::new(place).ok()?.read() else {
        return None;
    };
    if log.is_closed() {
        return None;
    }
    let order = |write: &Value| write["order"].as_u64();
    let writers = writers.as_object()?;
    Some(Written {
        tree: top,
        log: *log,
        last: Last {
            agent: writers.get(identity.within()).and_then(order),
            session: writers.values().filter_map(order).max().unwrap_or(0),
        },
    })
}

/// The worktree among `candidates` that the agent wrote in last, or, where it wrote in none of
/// them, the one its session wrote in last.
pub fn choose(candidates: impl IntoIterator<Item = (PathBuf, Last)>) -> Option<PathBuf> {
    candidates
        .into_iter()
        .max_by_key(|(_, last)| *last)
        .map(|(tree, _)| tree)
}

/// Records, where `agent` is a session's main agent, that it works in `dir`, so that a subagent
/// in another worktree is told apart from one in its main agent's.
pub fn note_main_dir(home: &Home, agent: &Agent, dir: &Path) -> Result<(), Failure> {
    if agent.subagent.is_some() {
        return Ok(());
    }
    let identity = Continuations::read(Some(home))?.identity(agent);
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_owned());
    let dir = json!(dir.display().to_string());
    home_json::set(home, HomeFile::SessionDirs, &identity.session_key(), dir)?;
    Ok(())
}

/// Whether `agent` is a subagent whose directory `dir` lies in a worktree other than its main
/// agent's, as its main agent last named it; not where that directory is not known.
fn in_own_worktree(home: &Home, agent: &Agent, dir: &Path) -> Result<bool, Failure> {
    if agent.subagent.is_none() {
        return Ok(false);
    }
    let identity = Continuations::read(Some(home))?.identity(agent);
    let dirs = home_json::object(home.read(HomeFile::SessionDirs)?.as_deref());
    let Some(main) = dirs.get(&identity.session_key()).and_then(Value::as_str) else {
        return Ok(false);
    };
    let top = |dir: &Path| {
        Place::locate(&YardGit::default(), dir)
            .ok()
            .map(|place| place.tree)
    };
    Ok(match (top(dir), top(Path::new(main))) {
        (Some(own), Some(main)) => own != main,
        _ => false,
    })
}

/// The agent's working tree: the worktree its session wrote in whose Yard Log holds its working
/// stage, else `dir`. A subagent in a worktree of its own follows only its own stage there.
pub fn resolve(home: Option<&Home>, agent: &Agent, dir: &Path) -> Result<PathBuf, Failure> {
    let Some(home) = home else {
        return Ok(dir.to_owned());
    };
    let own = in_own_worktree(home, agent, dir)?;
    let holding = written(home, agent)?
        .into_iter()
        .filter(|each| {
            if own {
                each.log.own_stage(agent, None).is_some()
            } else {
                each.log.working_stage(agent, None).is_some()
            }
        })
        .map(|each| (each.tree, each.last));
    Ok(choose(holding).unwrap_or_else(|| dir.to_owned()))
}

/// Whether a command follows the agent's work into its working tree, or stays in its directory
/// unless `--tree` names another.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Follows {
    Work,
    Directory,
}

/// The directory a command run in `dir` acts in: the one `--tree` names, where it names one;
/// else, for a command that follows the work of an agent it names, the agent's working tree;
/// else `dir`.
pub fn target(
    dir: &Path,
    named: Option<&Path>,
    agent: Option<&Agent>,
    follows: Follows,
) -> Result<PathBuf, Failure> {
    let home = Home::from_env();
    if let (Some(home), Some(agent)) = (&home, agent) {
        note_main_dir(home, agent, dir)?;
    }
    if let Some(named) = named {
        let path = dir.join(named);
        if !path.is_dir() {
            // A path that is no directory holds no worktree, as one outside git holds none.
            return Err(Failure::one(
                "repository",
                format!("--tree {}: no such directory", named.display()),
            ));
        }
        return Ok(path);
    }
    match agent {
        Some(agent) if follows == Follows::Work => resolve(home.as_ref(), agent, dir),
        _ => Ok(dir.to_owned()),
    }
}

/// Records, where `~/.yard/` can be reached, that `agent` wrote Yard Log in the worktree `tree`.
/// The write has been made by then, so a record that fails leaves it standing: the agent's
/// working tree is then found among the worktrees recorded before.
pub fn wrote(agent: &Agent, tree: &Path, at: Time) {
    if let Some(home) = Home::from_env() {
        record(&home, agent, tree, at).ok();
    }
}

/// Where a command that names something sent before acts, where `--tree` names no worktree:
/// the agent's working tree where its Yard Log holds what `holds` looks for; else the worktree
/// among those its session wrote in that holds it; else the working tree, which refuses it.
pub fn sent_to(
    dir: &Path,
    agent: &Agent,
    holds: impl Fn(&YardLog) -> bool,
) -> Result<PathBuf, Failure> {
    let working = target(dir, None, Some(agent), Follows::Work)?;
    let held = Place::locate(&YardGit::default(), &working)
        .ok()
        .and_then(|place| YardStore::new(place).ok())
        .and_then(|store| store.read().ok());
    if matches!(&held, Some(Held::Log(log)) if holds(log)) {
        return Ok(working);
    }
    Ok(holding(Home::from_env().as_ref(), agent, holds)?.unwrap_or(working))
}

/// The worktree among those `agent`'s session wrote in whose Yard Log `holds` says yes to.
pub fn holding(
    home: Option<&Home>,
    agent: &Agent,
    holds: impl Fn(&YardLog) -> bool,
) -> Result<Option<PathBuf>, Failure> {
    let Some(home) = home else {
        return Ok(None);
    };
    Ok(choose(
        written(home, agent)?
            .into_iter()
            .filter(|each| holds(&each.log))
            .map(|each| (each.tree, each.last)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(tree: &str, agent: Option<u64>, session: u64) -> (PathBuf, Last) {
        (PathBuf::from(tree), Last { agent, session })
    }

    #[test]
    fn the_worktree_the_agent_wrote_in_last_is_chosen() {
        let chosen = choose([at("/a", Some(3), 9), at("/b", Some(5), 5)]);
        assert_eq!(chosen, Some(PathBuf::from("/b")));
    }

    #[test]
    fn a_worktree_the_agent_wrote_in_is_chosen_over_one_only_its_session_wrote_in_later() {
        let chosen = choose([at("/a", None, 9), at("/b", Some(2), 2)]);
        assert_eq!(chosen, Some(PathBuf::from("/b")));
    }

    #[test]
    fn where_the_agent_wrote_in_none_the_session_s_last_worktree_is_chosen() {
        let chosen = choose([at("/a", None, 4), at("/b", None, 7)]);
        assert_eq!(chosen, Some(PathBuf::from("/b")));
    }

    #[test]
    fn no_candidate_chooses_no_worktree() {
        assert_eq!(choose(Vec::new()), None);
    }
}

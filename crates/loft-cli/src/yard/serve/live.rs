//! What the page server follows while it runs, four times a second through the clock port: a
//! newer `loft`'s request that it stop, which it heeds by finishing the responses it is sending
//! and exiting; and the files of every Yard Log it serves, whoever changes them, a `loft yard`
//! command or git, each change of which it counts so that open pages hear of it.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use loft_ports::clock::Clock as _;
use yardlog::codec::session::{ACTIVITY, TOKENS};
use yardlog_store::YARD;
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;
use yardlog_store::session::branches;

use super::Server;
use super::discovery;
use super::trees;
use crate::yard::clock;

/// How often the page server looks.
pub const TICK: Duration = Duration::from_millis(250);

/// The longest the page server waits for the responses it is sending before it exits.
const FINISHING: Duration = Duration::from_millis(600);

/// Follows the stop requests and the files until the page server exits.
pub fn follow(server: &Server) -> ! {
    let clock = clock();
    let mut last: Option<Digests> = None;
    let mut heads = HashMap::new();
    let mut tick: u64 = 0;
    loop {
        clock.wait(TICK);
        if discovery::asked_to_stop(&server.home) {
            stop(server);
        }
        tick += 1;
        if tick.is_multiple_of(2) {
            let now = fingerprint(&server.home, &mut heads);
            let changed = last.as_ref().map(|last| changed(last, &now));
            if let Some(changed) = changed.filter(|changed| !changed.is_empty()) {
                server.changed(changed);
            }
            last = Some(now);
        }
    }
}

/// What the page shows, by what of it changes together: the trees served and what each is on,
/// and the files of each branch's Yard Log, apart from those its sessions keep.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Area {
    Trees,
    Log { tree: String, branch: String },
    Sessions { tree: String, branch: String },
}

type Digests = BTreeMap<Area, u64>;

/// The areas whose digest differs between `last` and `now`, or that one of them lacks.
fn changed(last: &Digests, now: &Digests) -> Vec<Area> {
    let mut areas: Vec<Area> = now
        .iter()
        .filter(|(area, digest)| last.get(area) != Some(digest))
        .map(|(area, _)| area.clone())
        .collect();
    areas.extend(last.keys().filter(|area| !now.contains_key(area)).cloned());
    areas
}

/// Finishes the responses being sent, then exits; the operating system frees the lock.
fn stop(server: &Server) -> ! {
    server.stopping.store(true, Ordering::SeqCst);
    let clock = clock();
    let began = clock.elapsed();
    while server.busy.load(Ordering::SeqCst) > 0
        && clock.elapsed().saturating_sub(began) < FINISHING
    {
        clock.wait(Duration::from_millis(10));
    }
    std::process::exit(0)
}

/// A digest of each area of what the page shows: the trees served, with the size and time of
/// each one's HEAD file, which a checkout or a commit rewrites; and, for each branch's Yard Log
/// under a tree's `.yard/`, those of its own files and, apart, of its sessions' files. Where each
/// tree's HEAD file lies is asked of git once, through its port, and kept in `heads`, so that a
/// look runs no git command.
pub fn fingerprint(home: &Home, heads: &mut HashMap<PathBuf, Option<PathBuf>>) -> Digests {
    let mut digests = Digests::new();
    let mut listed = DefaultHasher::new();
    for tree in trees::served(home).unwrap_or_default() {
        tree.hash(&mut listed);
        let head = heads
            .entry(tree.clone())
            .or_insert_with(|| YardGit::default().git_path(&tree, "HEAD").ok());
        if let Some(head) = head {
            stamp(head, &mut listed);
        }
        let shown = tree.display().to_string();
        for branch in branches(&tree).unwrap_or_default() {
            let folder = tree.join(YARD).join(&branch);
            let (tree, branch) = (shown.clone(), branch.clone());
            let mut sessions = DefaultHasher::new();
            files(&folder.join(TOKENS), &mut sessions);
            files(&folder.join(ACTIVITY), &mut sessions);
            let mut log = DefaultHasher::new();
            files(&folder, &mut log);
            let log_area = Area::Log {
                tree: tree.clone(),
                branch: branch.clone(),
            };
            digests.insert(log_area, log.finish());
            digests.insert(Area::Sessions { tree, branch }, sessions.finish());
        }
    }
    digests.insert(Area::Trees, listed.finish());
    digests
}

/// Hashes the size and modification time of a file.
fn stamp_of(meta: &fs::Metadata, hasher: &mut DefaultHasher) {
    meta.len().hash(hasher);
    // The file's own stamp, as the file system keeps it: not a reading of the clock.
    meta.modified().ok().hash(hasher);
}

/// Hashes the size and modification time of the file at `path`, where there is one.
fn stamp(path: &Path, hasher: &mut DefaultHasher) {
    if let Ok(meta) = fs::metadata(path) {
        stamp_of(&meta, hasher);
    }
}

/// Hashes the name, size and modification time of each file right in `dir`.
fn files(dir: &Path, hasher: &mut DefaultHasher) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_file() {
            entry.file_name().hash(hasher);
            stamp_of(&meta, hasher);
        }
    }
}

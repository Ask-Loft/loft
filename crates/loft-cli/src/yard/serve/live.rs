//! What the page server follows while it runs, four times a second through the clock port: a
//! newer `loft`'s request that it stop, which it heeds by finishing the responses it is sending
//! and exiting; and the files of every Yard Log it serves, whoever changes them, a `loft yard`
//! command or git, each change of which it counts so that open pages hear of it.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use loft_ports::clock::Clock as _;
use yardlog_store::YARD;
use yardlog_store::git::YardGit;
use yardlog_store::home::{Home, HomeFile};

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
    let mut last = None;
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
            if last.is_some_and(|last| last != now) {
                server.changes.fetch_add(1, Ordering::SeqCst);
            }
            last = Some(now);
        }
    }
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

/// A digest of what the page shows: the trees served, the size and time of each one's HEAD
/// file, which a checkout or a commit rewrites, and those of every file under its `.yard/`.
/// Where each tree's HEAD file lies is asked of git once, through its port, and kept in `heads`,
/// so that a look runs no git command.
pub fn fingerprint(home: &Home, heads: &mut HashMap<PathBuf, Option<PathBuf>>) -> u64 {
    let mut hasher = DefaultHasher::new();
    home.read(HomeFile::Trees).ok().flatten().hash(&mut hasher);
    for tree in trees::served(home).unwrap_or_default() {
        tree.hash(&mut hasher);
        let head = heads
            .entry(tree.clone())
            .or_insert_with(|| YardGit::default().git_path(&tree, "HEAD").ok());
        if let Some(head) = head {
            stamp(head, &mut hasher);
        }
        files(&tree.join(YARD), &mut hasher);
    }
    hasher.finish()
}

/// Hashes the size and modification time of the file at `path`, where there is one.
fn stamp(path: &Path, hasher: &mut DefaultHasher) {
    if let Ok(meta) = fs::metadata(path) {
        meta.len().hash(hasher);
        // The file's own stamp, as the file system keeps it: not a reading of the clock.
        format!("{:?}", meta.modified().ok()).hash(hasher);
    }
}

fn files(dir: &Path, hasher: &mut DefaultHasher) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        path.hash(hasher);
        if path.is_dir() {
            files(&path, hasher);
        } else {
            stamp(&path, hasher);
        }
    }
}

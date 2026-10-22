//! What the page server follows while it runs, four times a second through the clock port: a
//! newer `loft`'s request that it stop, which it heeds by finishing the responses it is sending
//! and exiting; and the files of every Yard Log it serves, whoever changes them, a `loft yard`
//! command or git, each change of which it counts so that open pages hear of it.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
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
    let mut tick: u64 = 0;
    loop {
        clock.wait(TICK);
        if discovery::asked_to_stop(&server.home) {
            stop(server);
        }
        tick += 1;
        if tick.is_multiple_of(2) {
            let now = fingerprint(&server.home);
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

/// A digest of what the page shows: the trees served, the branch each is on, and the size and
/// time of every file under each tree's `.yard/`.
pub fn fingerprint(home: &Home) -> u64 {
    let mut hasher = DefaultHasher::new();
    home.read(HomeFile::Trees).ok().flatten().hash(&mut hasher);
    let git = YardGit::default();
    for tree in trees::served(home).unwrap_or_default() {
        tree.hash(&mut hasher);
        git.branch(&tree).ok().flatten().hash(&mut hasher);
        files(&tree.join(YARD), &mut hasher);
    }
    hasher.finish()
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
        } else if let Ok(meta) = entry.metadata() {
            meta.len().hash(hasher);
            // The file's own stamp, as the file system keeps it: not a reading of the clock.
            format!("{:?}", meta.modified().ok()).hash(hasher);
        }
    }
}

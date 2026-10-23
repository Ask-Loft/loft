//! What the store tests share: scratch directories, a worktree that the git simulator scripts,
//! Yard Log written through batches of JSON, and a helper process that holds a lock.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs, process, thread};

use serde_json::Value;
use software_plant::GitScenario;
use yardlog::YardLog;
use yardlog::act::{Agent, Time};
use yardlog::input::read_batch;
use yardlog::write::{Context, Refusal, Written};
use yardlog_store::git::YardGit;
use yardlog_store::{Place, Updated, YardStore, YardStoreError};

/// A directory removed when the test ends, named as the file system resolves it.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("yardlog-store-{}-{number}", process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }

    pub fn put(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    pub fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        snapshot(&self.0)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Every file under `root` by its path from `root`, and every directory as an entry ending in
/// `/` with no bytes.
pub fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut entries = BTreeMap::new();
    if !root.exists() {
        return entries;
    }
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                entries.insert(format!("{relative}/"), Vec::new());
                pending.push(path);
            } else {
                entries.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    entries
}

/// Git as the simulator answers it from `scenario`, installed in `plant`.
pub fn simulated(scenario: &GitScenario, plant: &Path) -> YardGit {
    let installed = scenario.install(plant);
    YardGit::from_command(move || {
        let mut command = Command::new("git");
        installed.apply(&mut command);
        command
    })
}

/// A worktree whose top is `tree/`, with its git directory at `tree/.git`, scripted by the
/// simulator.
pub struct Tree {
    pub scratch: Scratch,
    pub plant: Scratch,
}

impl Tree {
    pub fn new() -> Self {
        let scratch = Scratch::new();
        fs::create_dir_all(scratch.0.join("tree/.git")).unwrap();
        Self {
            scratch,
            plant: Scratch::new(),
        }
    }

    pub fn top(&self) -> PathBuf {
        self.scratch.0.join("tree")
    }

    pub fn git_dir(&self) -> PathBuf {
        self.top().join(".git")
    }

    /// The scenario of this worktree on `branch`, with its git directory where it lies.
    pub fn on(&self, branch: &str) -> GitScenario {
        let mut scenario = GitScenario::clean_on(branch);
        scenario.repository.as_mut().unwrap().git_dir =
            Some(self.git_dir().to_string_lossy().into_owned());
        scenario
    }

    pub fn place(&self, scenario: &GitScenario, dir: &Path) -> Result<Place, YardStoreError> {
        Place::locate(&simulated(scenario, &self.plant.0), dir)
    }

    /// The store of `branch`'s Yard Log, called from the top of the worktree.
    pub fn store(&self, branch: &str) -> YardStore {
        YardStore::new(self.place(&self.on(branch), &self.top()).unwrap()).unwrap()
    }
}

/// 2026-10-22T12:00:00Z plus `seconds`.
pub fn at(seconds: u64) -> Time {
    Time::from_unix_seconds(1_792_670_400 + seconds).unwrap()
}

pub fn agent() -> Agent {
    Agent::main("session-a")
}

/// Why a test's write failed.
#[derive(Debug)]
pub enum Failed {
    Store(YardStoreError),
    Refused(Refusal),
}

impl From<YardStoreError> for Failed {
    fn from(error: YardStoreError) -> Self {
        Self::Store(error)
    }
}

/// Writes a batch to the store's Yard Log, opening it where the branch has none.
pub fn try_write(store: &YardStore, batch: &Value) -> Result<Updated<Written>, Failed> {
    let inputs = read_batch(batch).unwrap_or_else(|faults| panic!("{faults:?}"));
    let branch = store.branch().to_owned();
    store.update(|current| {
        let mut log = current.unwrap_or_else(|| YardLog::open(&branch, "develop", at(0), agent()));
        let context = Context {
            at: at(log.root.revision + 1),
            agent: agent(),
            base: None,
            send: false,
        };
        let written = log
            .write(&inputs, &context, &mut |_, _| Err("no loft".to_owned()))
            .map_err(Failed::Refused)?;
        Ok((log, written))
    })
}

pub fn write(store: &YardStore, batch: &Value) -> Updated<Written> {
    try_write(store, batch).unwrap_or_else(|failed| panic!("{failed:?}"))
}

/// Yard Log as the store reads it.
pub fn read(store: &YardStore) -> YardLog {
    match store.read().unwrap() {
        yardlog_store::Held::Log(log) => *log,
        other => panic!("{other:?}"),
    }
}

/// The environment variable that tells this test binary, run again as a helper, what to do.
pub const HELPER_ENV: &str = "YARDLOG_STORE_HELPER";
/// The name of the test that does a helper's work.
pub const HELPER_TEST: &str = "concurrent_writes::helper";

/// This test binary, run again to take the lock of `path` and hold it until it is killed,
/// having written `ready` once it holds it.
pub fn holding(lock: &Path, ready: &Path) -> Child {
    let child = Command::new(env::current_exe().unwrap())
        .args([HELPER_TEST, "--exact", "--include-ignored", "--nocapture"])
        .env(
            HELPER_ENV,
            format!("hold\n{}\n{}", lock.display(), ready.display()),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for(ready);
    child
}

/// Waits, a few seconds at most, for `path` to be there.
pub fn wait_for(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "{} never came",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

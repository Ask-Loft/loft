//! Putting a simulator in front of the binary under test.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::Hasher;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::OnceLock;

use crate::git::{GitScenario, LOCKS_FILE, SCENARIO_ENV};

/// How often a build is retried when another test process is re-linking its output.
const ATTEMPTS: usize = 5;

/// The simulator's `git` executable, built on first use by the Cargo that builds the tests and
/// copied, named by its content, to a directory of its own beside Cargo's output. Cargo re-links
/// its own output on every build, even one with nothing to do, so test processes building at
/// once would otherwise run a file that is briefly missing.
///
/// # Panics
///
/// The executable cannot be built or copied.
pub fn git_executable() -> &'static Path {
    static GIT: OnceLock<PathBuf> = OnceLock::new();
    GIT.get_or_init(|| {
        let mut last = None;
        for _ in 0..ATTEMPTS {
            match stable_copy(&build_git()) {
                Ok(path) => return path,
                Err(error) => last = Some(error),
            }
        }
        panic!("the git simulator cannot be copied: {last:?}")
    })
}

fn build_git() -> PathBuf {
    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "--quiet",
            "--package",
            "software-plant",
            "--bin",
            "git",
            "--message-format=json",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo runs");
    assert!(
        output.status.success(),
        "the git simulator builds\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|message| message["target"]["name"] == "git" && message["executable"].is_string())
        .and_then(|message| message["executable"].as_str().map(PathBuf::from))
        .expect("cargo names the simulator's executable")
}

/// Copies the executable to `software-plant/<content>/git` beside it, unless a copy of the same
/// content is there already, and names the copy.
fn stable_copy(built: &Path) -> io::Result<PathBuf> {
    let bytes = fs::read(built)?;
    let mut hasher = DefaultHasher::new();
    hasher.write(&bytes);
    let dir = built
        .parent()
        .expect("the executable lies in a directory")
        .join("software-plant")
        .join(format!("{:016x}", hasher.finish()));
    let path = dir.join("git");
    if !path.is_file() {
        fs::create_dir_all(&dir)?;
        let partial = dir.join(format!("git.{}.partial", process::id()));
        fs::copy(built, &partial)?;
        fs::rename(&partial, &path)?;
    }
    Ok(path)
}

/// A scenario written into a directory, with the simulator first on the search path.
#[derive(Debug, Clone)]
pub struct Installed {
    pub scenario: PathBuf,
    pub search_path: PathBuf,
}

impl GitScenario {
    /// Writes the scenario into `dir` and names the directory that holds the simulator.
    #[must_use]
    pub fn install(&self, dir: &Path) -> Installed {
        let scenario = dir.join("git-scenario.json");
        self.write(&scenario);
        let _ = fs::remove_file(dir.join(LOCKS_FILE));
        let search_path = git_executable()
            .parent()
            .expect("the executable lies in a directory")
            .to_path_buf();
        Installed {
            scenario,
            search_path,
        }
    }
}

impl Installed {
    /// The locks the simulator recorded since the scenario was installed, in the order the calls
    /// took them.
    #[must_use]
    pub fn locks(&self) -> Vec<String> {
        fs::read_to_string(self.scenario.with_file_name(LOCKS_FILE))
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// Gives `command` a search path whose only `git` is the simulator, and the scenario.
    pub fn apply<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        command
            .env("PATH", &self.search_path)
            .env(SCENARIO_ENV, &self.scenario)
    }
}

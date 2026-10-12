//! Putting a simulator in front of the binary under test.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use crate::git::{GitScenario, SCENARIO_ENV};

/// The simulator's `git` executable, built on first use by the Cargo that builds the tests.
///
/// # Panics
///
/// The executable cannot be built.
pub fn git_executable() -> &'static Path {
    static GIT: OnceLock<PathBuf> = OnceLock::new();
    GIT.get_or_init(build_git)
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
    /// Gives `command` a search path whose only `git` is the simulator, and the scenario.
    pub fn apply<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        command
            .env("PATH", &self.search_path)
            .env(SCENARIO_ENV, &self.scenario)
    }
}

//! Real git for the git contract suites, kept from the user's configuration, and what the
//! suites compare of it and the simulator.

use std::path::Path;
use std::process::{Command, Output};

use loftspec::publish::RepoState;
use loftspec_store::StoreError;
use loftspec_store::repository::{GitCommand, Repository as _};
use software_plant::GitScenario;

/// Real git, kept from the user's configuration and from any repository above the temporary
/// directory.
pub fn real_git() -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CEILING_DIRECTORIES",
            std::env::temp_dir().canonicalize().unwrap(),
        );
    command
}

/// Runs a setup command of real git in `dir`. Automatic maintenance is off: a commit would
/// otherwise leave a detached `git maintenance` process writing under `.git/` while a test
/// compares the repository's files.
pub fn git(dir: &Path, args: &[&str]) {
    let output = real_git()
        .args([
            "-c",
            "user.name=Plant",
            "-c",
            "user.email=plant@example.invalid",
            "-c",
            "maintenance.auto=false",
        ])
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// What one call printed and how it exited; the error message only where git exits 128, the
/// one failure whose message Loft reads.
pub fn reply(output: &Output) -> (Option<i32>, Vec<u8>, String) {
    let code = output.status.code();
    let stderr = if code == Some(128) {
        String::from_utf8_lossy(&output.stderr).into_owned()
    } else {
        String::new()
    };
    (code, output.stdout.clone(), stderr)
}

/// The port's answer, with an error reduced to what the publisher acts on.
pub fn state(git: &GitCommand, root: &Path) -> Result<RepoState, String> {
    git.state(root).map_err(|error| match error {
        StoreError::NotARepository(_) => "not a repository".to_owned(),
        other => other.to_string(),
    })
}

pub fn simulated(scenario: &GitScenario, plant: &Path) -> GitCommand {
    let simulator = scenario.install(plant);
    GitCommand::from_command(move || {
        let mut command = Command::new("git");
        simulator.apply(&mut command);
        command
    })
}

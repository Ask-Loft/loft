//! Real git for the git contract suites, kept from the user's configuration, and what the
//! suites compare of it and the simulator.

use std::path::Path;
use std::process::{Command, Output};

use loftspec::publish::RepoState;
use loftspec_store::StoreError;
use loftspec_store::repository::{GitCommand, Repository as _};
use software_plant::GitScenario;
use software_plant::real_git::setup_git;

pub use software_plant::real_git::real_git;

/// Runs a setup command of real git in `dir`, giving its output.
pub fn git_output(dir: &Path, args: &[&str]) -> Output {
    setup_git().arg("-C").arg(dir).args(args).output().unwrap()
}

/// Runs a setup command of real git in `dir`, which succeeds.
pub fn git(dir: &Path, args: &[&str]) {
    let output = git_output(dir, args);
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

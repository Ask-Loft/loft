//! Real git, as the contract suites run it beside the simulator: kept from the user's
//! configuration and from any repository above the temporary directory.

use std::env;
use std::process::Command;

/// Keeps a command that runs git, or runs Loft which runs git, from the user's git
/// configuration and from any repository above the temporary directory.
///
/// # Panics
///
/// The temporary directory cannot be resolved.
pub fn isolate(command: &mut Command) -> &mut Command {
    let ceiling = env::temp_dir()
        .canonicalize()
        .expect("the temporary directory resolves");
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CEILING_DIRECTORIES", ceiling)
}

/// Real git, isolated.
#[must_use]
pub fn real_git() -> Command {
    let mut command = Command::new("git");
    isolate(&mut command);
    command
}

/// Real git, isolated, as a suite runs it to build a repository: with an identity to commit as,
/// and no automatic maintenance, which would leave a detached `git maintenance` process writing
/// under `.git/` while a test compares the repository's files.
#[must_use]
pub fn setup_git() -> Command {
    let mut command = real_git();
    command.args([
        "-c",
        "user.name=Plant",
        "-c",
        "user.email=plant@example.invalid",
        "-c",
        "maintenance.auto=false",
    ]);
    command
}

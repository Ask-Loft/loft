//! Real git, as the contract suites run it beside the simulator: kept from the user's
//! configuration, from any repository above the temporary directory and from any repository
//! git's own environment names, and run directly rather than through a launcher in front of it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::{env, iter};

/// The directory that holds git's own executable, as `git --exec-path` names it. The `git` on
/// the search path can be a launcher that finds git anew on every call, as `/usr/bin/git` is on
/// macOS, which doubles the time of a suite that runs git hundreds of times.
///
/// # Panics
///
/// git cannot be run or does not name its directory.
fn exec_path() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let output = Command::new("git")
            .arg("--exec-path")
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git names its exec path");
        let dir = String::from_utf8(output.stdout).expect("the exec path is UTF-8");
        PathBuf::from(dir.trim_end())
    })
}

/// The variables through which git finds a repository other than the one around its working
/// directory, `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` and the rest, as
/// `git rev-parse --local-env-vars` names them. git sets them for the commands it runs itself,
/// such as a hook or `git rebase --exec`, so a test started that way would otherwise reach the
/// developer's own repository instead of its scratch one.
///
/// # Panics
///
/// git cannot be run or does not list them.
fn repository_variables() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let output = Command::new(exec_path().join("git"))
            .args(["rev-parse", "--local-env-vars"])
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git lists its repository variables"
        );
        let names = String::from_utf8(output.stdout).expect("the names are UTF-8");
        names.lines().map(str::to_owned).collect()
    })
}

/// Keeps a command from any repository git's environment names, so that git, run by it or by
/// Loft under it, finds only the repository around its working directory.
///
/// # Panics
///
/// git cannot be run or does not list its repository variables.
pub fn forget_repository(command: &mut Command) -> &mut Command {
    for name in repository_variables() {
        command.env_remove(name);
    }
    command
}

/// Keeps a command that runs git, or runs Loft which runs git, from the user's git
/// configuration, from any repository above the temporary directory and from any repository
/// git's environment names, and puts git's own directory first on its search path.
///
/// # Panics
///
/// The temporary directory cannot be resolved, or git's directory cannot be found.
pub fn isolate(command: &mut Command) -> &mut Command {
    let ceiling = env::temp_dir()
        .canonicalize()
        .expect("the temporary directory resolves");
    let inherited = env::var_os("PATH").unwrap_or_default();
    let search = iter::once(exec_path().to_path_buf()).chain(env::split_paths(&inherited));
    let search = env::join_paths(search).expect("the search path joins");
    forget_repository(command)
        .env("PATH", search)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CEILING_DIRECTORIES", ceiling)
}

/// Real git, isolated.
#[must_use]
pub fn real_git() -> Command {
    let mut command = Command::new(exec_path().join("git"));
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

/// Runs a setup command of [`setup_git`] in `dir`, giving its output.
///
/// # Panics
///
/// git cannot be run.
#[must_use]
pub fn git_output(dir: &Path, args: &[&str]) -> Output {
    setup_git().arg("-C").arg(dir).args(args).output().unwrap()
}

/// Runs a setup command of [`setup_git`] in `dir`, which succeeds.
///
/// # Panics
///
/// git cannot be run, or the command fails.
pub fn git(dir: &Path, args: &[&str]) {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

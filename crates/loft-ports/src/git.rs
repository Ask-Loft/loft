//! The git runner: every call Loft makes of git runs `git -C <directory> <call>` through it,
//! with git's optional locks off, so that reading a repository never takes the index lock (the
//! accepted decision "Git is an external component reached through the `git` command"). Tests
//! give it the Software Plant's simulator in place of git.

use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;

use crate::PortError;

/// What git prints, with exit code 128, in a directory outside any repository.
pub const NOT_A_REPOSITORY: &str = "not a git repository";
/// The checked-out branch; exit 1 on a detached HEAD.
pub const CURRENT_BRANCH: [&str; 4] = ["symbolic-ref", "--quiet", "--short", "HEAD"];
/// A path under the worktree's own git directory, from the working directory, completed by it.
pub const GIT_PATH: [&str; 2] = ["rev-parse", "--git-path"];

/// Runs git.
pub struct Git {
    command: Box<dyn Fn() -> Command>,
}

impl Default for Git {
    /// Runs the `git` on the search path.
    fn default() -> Self {
        Self::from_command(|| Command::new("git"))
    }
}

impl Git {
    /// Runs the git command that `command` starts, with the arguments of each call added.
    pub fn from_command(command: impl Fn() -> Command + 'static) -> Self {
        Self {
            command: Box::new(command),
        }
    }

    /// The command of one call in `dir`.
    fn call(&self, dir: &Path, call: &[&str]) -> Command {
        let mut command = (self.command)();
        command
            .arg("-C")
            .arg(dir)
            .args(call)
            .env("GIT_OPTIONAL_LOCKS", "0");
        command
    }

    /// Runs one call in `dir`, and gives what it printed and how it exited.
    ///
    /// # Errors
    ///
    /// git cannot be run.
    pub fn run(&self, dir: &Path, call: &[&str]) -> Result<Output, PortError> {
        self.call(dir, call).output().map_err(spawn_error(call))
    }

    /// Runs one call with `input` on its standard input, written from another thread so that a
    /// call that answers while it reads cannot stall on a full pipe.
    ///
    /// # Errors
    ///
    /// git cannot be run.
    pub fn run_with_input(
        &self,
        dir: &Path,
        call: &[&str],
        input: &[u8],
    ) -> Result<Output, PortError> {
        let mut child = self
            .call(dir, call)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(spawn_error(call))?;
        let mut stdin = child.stdin.take().expect("the input is piped");
        thread::scope(|scope| {
            scope.spawn(move || stdin.write_all(input));
            child.wait_with_output()
        })
        .map_err(spawn_error(call))
    }
}

fn spawn_error<'a>(call: &'a [&'a str]) -> impl FnOnce(io::Error) -> PortError + 'a {
    move |error| match error.kind() {
        io::ErrorKind::NotFound => PortError::GitUnavailable(error.to_string()),
        _ => PortError::Git {
            call: call.join(" "),
            message: error.to_string(),
        },
    }
}

/// What a call printed, where it succeeded; its call and message where it failed.
///
/// # Errors
///
/// The call exited with a code other than 0.
pub fn succeeded(call: &[&str], output: Output) -> Result<Vec<u8>, PortError> {
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(PortError::Git {
            call: call.join(" "),
            message: String::from_utf8_lossy(&output.stderr)
                .trim_end()
                .to_owned(),
        })
    }
}

/// Whether a call that answers with its exit code alone says yes (0) or no (1).
///
/// # Errors
///
/// The call exited with another code.
pub fn yes_or_no(call: &[&str], output: Output) -> Result<bool, PortError> {
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    succeeded(call, output).map(|_| true)
}

/// What a call printed, where it succeeded, and none where it exited 1, as a call that finds
/// nothing does.
///
/// # Errors
///
/// The call exited with another code than 0 or 1.
pub fn if_found(call: &[&str], output: Output) -> Result<Option<Vec<u8>>, PortError> {
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    succeeded(call, output).map(Some)
}

/// Whether a call failed because its directory lies in no repository.
#[must_use]
pub fn outside_a_repository(output: &Output) -> bool {
    output.status.code() == Some(128)
        && String::from_utf8_lossy(&output.stderr).contains(NOT_A_REPOSITORY)
}

/// The first line of a call's output.
#[must_use]
pub fn first_line(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}
